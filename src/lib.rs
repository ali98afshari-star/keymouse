//! keymouse — hold one key, move the mouse with the arrow keys (X11).
//!
//! v2.0: pure-Rust rewrite (x11rb, no Python, no interpreter).
//! The library holds the config, the keysym table, the pure movement
//! logic and the X11 helpers; the `keymouse` binary is the daemon and
//! `keymouse-settings` is the egui settings window.

pub mod config;
pub mod keysym;
pub mod logic;
pub mod x11util;

pub use config::Config;

/// Daemon data directory: lock file, log.
pub fn data_dir() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("keymouse")
}
