//! GUI smoke test: run `keymouse-settings` under Xvfb, verify that
//! (a) a real window appears, and (b) the app renders and saves the
//! config (via the hidden --smoke-test hook: render a few frames,
//! save, exit).
//!
//! Run: cargo test --release --test gui_smoke

#[path = "common/mod.rs"]
mod common;

use common::{TempHome, Xvfb};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::rust_connection::RustConnection;

fn seed_config(home: &TempHome) {
    let dir = home.path.join(".config").join("keymouse");
    std::fs::create_dir_all(&dir).expect("config dir");
    // A distinctive value so we can prove the GUI loaded + saved it.
    std::fs::write(
        dir.join("config.toml"),
        "base_speed = 9.0\nmodifier = \"Control_R\"\n",
    )
    .expect("seed config");
}

fn config_text(home: &TempHome) -> String {
    std::fs::read_to_string(
        home.path.join(".config").join("keymouse").join("config.toml"),
    )
    .expect("config file exists")
}

fn start_settings(xvfb: &Xvfb, home: &TempHome, smoke: bool) -> Child {
    let bin = env!("CARGO_BIN_EXE_keymouse-settings");
    let mut cmd = Command::new(bin);
    cmd.env("DISPLAY", &xvfb.display).env("HOME", &home.path);
    if smoke {
        cmd.arg("--smoke-test");
    }
    cmd.stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start keymouse-settings")
}

/// Find a top-level window whose WM_NAME contains "KeyMouse".
fn find_settings_window(conn: &RustConnection, root: Window) -> Option<(Window, u16, u16)> {
    let tree = conn.query_tree(root).ok()?.reply().ok()?;
    for child in tree.children {
        let name = conn
            .get_property(false, child, AtomEnum::WM_NAME, AtomEnum::STRING, 0, 64)
            .ok()?
            .reply()
            .ok()?;
        let text = String::from_utf8_lossy(&name.value).into_owned();
        if text.contains("KeyMouse") {
            let geo = conn.get_geometry(child).ok()?.reply().ok()?;
            return Some((child, geo.width, geo.height));
        }
    }
    None
}

#[test]
fn settings_renders_and_saves_under_xvfb() {
    let xvfb = Xvfb::start();
    let home = TempHome::new("gui");

    // -- part A: --smoke-test renders frames and saves the config ------
    seed_config(&home);
    let mut app = start_settings(&xvfb, &home, true);
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Ok(Some(s)) = app.try_wait() {
            break s;
        }
        if Instant::now() > deadline {
            let _ = app.kill();
            panic!("keymouse-settings --smoke-test did not exit in 20 s");
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    assert!(status.success(), "--smoke-test exited with {status}");
    let saved = config_text(&home);
    assert!(
        saved.contains("base_speed = 9"),
        "config was not saved through the GUI (got: {saved})"
    );

    // -- part B: a real window appears ----------------------------------
    let mut app = start_settings(&xvfb, &home, false);
    let (conn, screen_num) = xvfb.connect();
    let root = conn.setup().roots[screen_num].root;
    let mut found = None;
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if let Some(w) = find_settings_window(&conn, root) {
            found = Some(w);
            break;
        }
        if let Ok(Some(s)) = app.try_wait() {
            panic!("keymouse-settings exited early with {s}");
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    let (_, w, h) = found.expect("no KeyMouse settings window appeared under Xvfb");
    assert!(w >= 400 && h >= 400, "settings window suspiciously small: {w}x{h}");

    let _ = app.kill();
    let _ = app.wait();
}
