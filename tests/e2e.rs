//! E2E: run the real `keymouse` daemon under Xvfb, fake keys via XTest,
//! and verify the pointer actually moves, a click registers, and
//! scroll events are produced.
//!
//! Run: cargo test --release --test e2e

#[path = "common/mod.rs"]
mod common;

use common::{fake_key, pointer_pos, TempHome, Xvfb};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::COPY_DEPTH_FROM_PARENT;

fn start_daemon(xvfb: &Xvfb, home: &TempHome) -> Child {
    let bin = env!("CARGO_BIN_EXE_keymouse");
    let child = Command::new(bin)
        .env("DISPLAY", &xvfb.display)
        .env("HOME", &home.path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start daemon");
    // Let it connect + grab the modifier key.
    std::thread::sleep(Duration::from_millis(800));
    child
}

/// Drain all pending events on this connection.
fn drain(conn: &impl Connection) {
    while let Ok(Some(_)) = conn.poll_for_event() {}
}

#[test]
fn daemon_moves_clicks_and_scrolls_under_xvfb() {
    let xvfb = Xvfb::start();
    let home = TempHome::new("daemon");
    let mut daemon = start_daemon(&xvfb, &home);
    let (conn, screen_num) = xvfb.connect();
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;

    // A small window under the pointer to receive button/scroll events.
    let win = conn.generate_id().expect("id");
    conn.create_window(
        COPY_DEPTH_FROM_PARENT,
        win,
        root,
        440,
        200,
        400,
        400,
        0,
        WindowClass::INPUT_OUTPUT,
        0,
        &CreateWindowAux::new()
            .background_pixel(screen.white_pixel)
            .event_mask(EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE),
    )
    .expect("create window");
    conn.map_window(win).expect("map window");
    let _ = conn.flush();
    std::thread::sleep(Duration::from_millis(300));

    let warp_center = |conn: &RustConnection| {
        let _ = conn.warp_pointer(x11rb::NONE, root, 0, 0, 0, 0, 640, 400);
        let _ = conn.flush();
        std::thread::sleep(Duration::from_millis(150));
    };

    // -- move up -------------------------------------------------------
    warp_center(&conn);
    let (x0, y0, _) = pointer_pos(&conn, root);
    fake_key(&conn, "Control_R", true);
    std::thread::sleep(Duration::from_millis(200));
    fake_key(&conn, "Up", true);
    std::thread::sleep(Duration::from_millis(350));
    fake_key(&conn, "Up", false);
    fake_key(&conn, "Control_R", false);
    std::thread::sleep(Duration::from_millis(200));
    let (x1, y1, _) = pointer_pos(&conn, root);
    assert!(y1 < y0 - 20, "pointer did not move up: ({x0},{y0}) -> ({x1},{y1})");
    assert!((x1 - x0).abs() <= 3, "pointer drifted sideways while moving up: dx={}", x1 - x0);

    // -- move right ------------------------------------------------------
    warp_center(&conn);
    let (x0, y0, _) = pointer_pos(&conn, root);
    fake_key(&conn, "Control_R", true);
    std::thread::sleep(Duration::from_millis(200));
    fake_key(&conn, "Right", true);
    std::thread::sleep(Duration::from_millis(350));
    fake_key(&conn, "Right", false);
    fake_key(&conn, "Control_R", false);
    std::thread::sleep(Duration::from_millis(200));
    let (x1, y1, _) = pointer_pos(&conn, root);
    assert!(x1 > x0 + 20, "pointer did not move right: ({x0},{y0}) -> ({x1},{y1})");
    assert!((y1 - y0).abs() <= 3, "pointer drifted vertically while moving right: dy={}", y1 - y0);

    // -- left click (Enter) while held: button 1 mask appears -----------
    warp_center(&conn);
    drain(&conn);
    fake_key(&conn, "Control_R", true);
    std::thread::sleep(Duration::from_millis(200));
    fake_key(&conn, "Return", true);
    std::thread::sleep(Duration::from_millis(200));
    let (_, _, mask_down) = pointer_pos(&conn, root);
    assert!(mask_down & 0x100 != 0, "button 1 not down while Enter held (mask={mask_down:#x})");
    fake_key(&conn, "Return", false);
    std::thread::sleep(Duration::from_millis(200));
    let (_, _, mask_up) = pointer_pos(&conn, root);
    assert!(mask_up & 0x100 == 0, "button 1 still down after Enter released (mask={mask_up:#x})");
    fake_key(&conn, "Control_R", false);
    std::thread::sleep(Duration::from_millis(200));

    // -- scroll up (PageUp) while held: button-4 events on the window ----
    warp_center(&conn);
    drain(&conn);
    fake_key(&conn, "Control_R", true);
    std::thread::sleep(Duration::from_millis(200));
    fake_key(&conn, "Prior", true);
    std::thread::sleep(Duration::from_millis(400));
    fake_key(&conn, "Prior", false);
    fake_key(&conn, "Control_R", false);
    // Collect button events for a moment.
    let mut scroll_ups = 0;
    let deadline = Instant::now() + Duration::from_millis(500);
    while Instant::now() < deadline {
        if let Ok(Some(Event::ButtonPress(ev))) = conn.poll_for_event() {
            // Button 4 = scroll up (XTest button press from the daemon).
            if ev.detail == 4 {
                scroll_ups += 1;
            }
        } else {
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    assert!(scroll_ups >= 1, "no scroll-up (button 4) events received, got {scroll_ups}");

    // -- daemon still alive ----------------------------------------------
    assert!(daemon.try_wait().expect("wait").is_none(), "daemon exited during the test");

    let _ = daemon.kill();
    let _ = daemon.wait();
}
