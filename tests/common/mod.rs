//! Shared helpers for the keymouse integration tests.
//! Starts a real Xvfb, connects with x11rb, fakes keys via XTest.

use std::process::{Child, Command, Stdio};
use std::time::Duration;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::xtest;
use x11rb::rust_connection::RustConnection;
use x11rb::CURRENT_TIME;

/// A running Xvfb server.
pub struct Xvfb {
    pub display: String,
    child: Child,
}

impl Xvfb {
    /// Start Xvfb on the first free display among :99, :98, :97.
    pub fn start() -> Xvfb {
        for n in [99, 98, 97, 96] {
            let display = format!(":{n}");
            let child = Command::new("Xvfb")
                .args([&display, "-screen", "0", "1280x800x24"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
            let Ok(mut child) = child else { continue };
            // Wait until we can connect.
            let mut ok = false;
            for _ in 0..50 {
                std::thread::sleep(Duration::from_millis(100));
                if let Ok((conn, _)) = x11rb::connect(Some(&display)) {
                    drop(conn);
                    ok = true;
                    break;
                }
                if let Ok(Some(_)) = child.try_wait() {
                    break; // Xvfb died (display probably taken)
                }
            }
            if ok {
                return Xvfb { display, child };
            }
            let _ = child.kill();
            let _ = child.wait();
        }
        panic!("could not start Xvfb on any display");
    }

    pub fn connect(&self) -> (RustConnection, usize) {
        x11rb::connect(Some(&self.display)).expect("connect to Xvfb")
    }
}

impl Drop for Xvfb {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// X core event type codes.
pub const KEY_PRESS: u8 = 2;
pub const KEY_RELEASE: u8 = 3;
pub const BUTTON_PRESS: u8 = 4;

/// Fake a key press/release via XTest.
pub fn fake_key(conn: &RustConnection, keysym_name: &str, press: bool) {
    let keymap = keymouse::x11util::Keymap::fetch(conn).expect("keymap");
    let kc = keymouse::x11util::keycode_for_name(&keymap, keysym_name)
        .unwrap_or_else(|| panic!("no keycode for keysym {keysym_name}"));
    let ty = if press { KEY_PRESS } else { KEY_RELEASE };
    let _ = xtest::fake_input(conn, ty, kc, CURRENT_TIME, 0, 0, 0, 0);
    let _ = conn.flush();
}

pub fn pointer_pos(conn: &RustConnection, root: Window) -> (i16, i16, u16) {
    let r = conn.query_pointer(root).expect("query_pointer").reply().expect("reply");
    (r.root_x, r.root_y, r.mask.into())
}

/// A fresh temp HOME so the daemon/settings use isolated config+lock files.
pub struct TempHome {
    pub path: std::path::PathBuf,
}

impl TempHome {
    pub fn new(tag: &str) -> TempHome {
        let path = std::env::temp_dir()
            .join(format!("keymouse-e2e-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create temp home");
        TempHome { path }
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
