//! keymouse-settings (v2, Rust + egui).
//!
//! A modern, dark settings window for keymouse:
//!  * Status — daemon running/stopped, start/stop, autostart toggle.
//!  * Keybindings — click a row, press a key, it is captured
//!    (game-style key mapping; Escape cancels the capture).
//!  * Pointer speed — sliders + live px/sec preview.
//!  * Behaviour — tick and scroll repeat.
//!  * Apply & Restart / Reset to defaults.
//!
//! The daemon binary is `keymouse`; this window only edits
//! ~/.config/keymouse/config.toml and (re)starts the daemon.

use keymouse::config::{config_path, save, Config};
use keymouse::keysym::{display_name, mirror_keysym, name_from_keysym};
use keymouse::x11util::Keymap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::CURRENT_TIME;

// -- palette (same as v1.1 tkinter) ------------------------------------
const BG: (u8, u8, u8) = (0x1e, 0x22, 0x2b);
const PANEL: (u8, u8, u8) = (0x28, 0x2d, 0x38);
const ACCENT: (u8, u8, u8) = (0x2d, 0xd4, 0xbf);
const FG: (u8, u8, u8) = (0xe8, 0xea, 0xf0);
const MUTED: (u8, u8, u8) = (0x9a, 0xa3, 0xb2);
const OK: (u8, u8, u8) = (0x4a, 0xde, 0x80);
const DANGER: (u8, u8, u8) = (0xf8, 0x71, 0x71);

fn daemon_bin() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let local = PathBuf::from(&home).join(".local").join("bin").join("keymouse");
    if local.exists() {
        return local;
    }
    let usr = PathBuf::from("/usr/local/bin/keymouse");
    if usr.exists() {
        return usr;
    }
    local
}

fn daemon_running() -> bool {
    Command::new("pgrep")
        .args(["-x", "keymouse"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn daemon_stop() {
    let _ = Command::new("pkill")
        .args(["-x", "keymouse"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    for _ in 0..20 {
        if !daemon_running() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn daemon_start() {
    let bin = daemon_bin();
    let log = keymouse::data_dir().join("keymouse.log");
    let _ = std::fs::create_dir_all(log.parent().unwrap());
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log);
    let (stdout, stderr) = match file {
        Ok(f) => {
            let f2 = f.try_clone().ok();
            (Stdio::from(f), f2.map(Stdio::from).unwrap_or(Stdio::null()))
        }
        Err(_) => (Stdio::null(), Stdio::null()),
    };
    let _ = Command::new(&bin)
        .stdout(stdout)
        .stderr(stderr)
        .stdin(Stdio::null())
        .spawn();
}

fn autostart_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("autostart")
        .join("keymouse.desktop")
}

fn autostart_enabled() -> bool {
    autostart_path().exists()
}

fn set_autostart(enabled: bool) {
    let path = autostart_path();
    if enabled {
        let _ = std::fs::create_dir_all(path.parent().unwrap());
        let exe = daemon_bin();
        let _ = std::fs::write(
            &path,
            format!(
                "[Desktop Entry]\nType=Application\nName=KeyMouse\n\
                 Comment=Hold a key and move the mouse with the arrow keys\n\
                 Exec={}\nTerminal=false\nX-XFCE-Autostart-enabled=true\n",
                exe.display()
            ),
        );
    } else {
        let _ = std::fs::remove_file(&path);
    }
}

/// Which binding is being captured right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureTarget {
    Modifier,
    Up,
    Down,
    Left,
    Right,
    Fast,
    Slow,
    LeftClick,
    RightClick,
    MiddleClick,
    ScrollUp,
    ScrollDown,
    Cancel,
}

impl CaptureTarget {
    fn label(self) -> &'static str {
        match self {
            CaptureTarget::Modifier => "Modifier (hold key)",
            CaptureTarget::Up => "Move up",
            CaptureTarget::Down => "Move down",
            CaptureTarget::Left => "Move left",
            CaptureTarget::Right => "Move right",
            CaptureTarget::Fast => "Fast",
            CaptureTarget::Slow => "Slow / precise",
            CaptureTarget::LeftClick => "Left click",
            CaptureTarget::RightClick => "Right click",
            CaptureTarget::MiddleClick => "Middle click",
            CaptureTarget::ScrollUp => "Scroll up",
            CaptureTarget::ScrollDown => "Scroll down",
            CaptureTarget::Cancel => "Cancel",
        }
    }
}

/// A live X11 connection used only while capturing a key.
struct CaptureConn {
    conn: RustConnection,
    keymap: Keymap,
}

impl CaptureConn {
    fn open() -> Result<CaptureConn, String> {
        let (conn, _) = x11rb::connect(None).map_err(|e| e.to_string())?;
        let keymap = Keymap::fetch(&conn).map_err(|e| e.to_string())?;
        let screen = &conn.setup().roots[0];
        // Grab the keyboard so the pressed key comes to us (and nowhere else).
        conn.grab_keyboard(false, screen.root, CURRENT_TIME, GrabMode::ASYNC, GrabMode::ASYNC)
            .map_err(|e| format!("cannot grab keyboard: {e}"))?;
        let _ = conn.flush();
        Ok(CaptureConn { conn, keymap })
    }

    /// Non-blocking poll: returns the keysym NAME of a pressed key, if any.
    /// Returns Some("") for Escape (cancel), None if nothing yet.
    fn poll_key(&self) -> Option<Option<String>> {
        let ev = match self.conn.poll_for_event() {
            Ok(Some(Event::KeyPress(ev))) => ev,
            _ => return None,
        };
        let Some(keysym) = self.keymap.first_keysym(ev.detail) else {
            return Some(None);
        };
        if keysym == 0xFF1B {
            // Escape cancels the capture.
            return Some(Some(String::new()));
        }
        let name = name_from_keysym(keysym);
        Some(Some(name))
    }
}

impl Drop for CaptureConn {
    fn drop(&mut self) {
        let _ = self.conn.ungrab_keyboard(CURRENT_TIME);
        let _ = self.conn.flush();
    }
}

struct SettingsApp {
    cfg: Config,
    daemon_running: bool,
    autostart: bool,
    capturing: Option<CaptureTarget>,
    capture: Option<CaptureConn>,
    message: String,
    last_status_check: Instant,
    /// Hidden test hook: render a few frames, save the config, exit.
    smoke_test: bool,
    frames: u32,
}

impl SettingsApp {
    fn new(smoke_test: bool) -> SettingsApp {
        SettingsApp {
            cfg: Config::load(None),
            daemon_running: daemon_running(),
            autostart: autostart_enabled(),
            capturing: None,
            capture: None,
            message: String::new(),
            last_status_check: Instant::now(),
            smoke_test,
            frames: 0,
        }
    }

    fn refresh_status(&mut self) {
        self.daemon_running = daemon_running();
        self.autostart = autostart_enabled();
    }

    fn binding_value(&self, target: CaptureTarget) -> String {
        match target {
            CaptureTarget::Modifier => display_name(&self.cfg.modifier),
            CaptureTarget::Up => display_name(&self.cfg.up),
            CaptureTarget::Down => display_name(&self.cfg.down),
            CaptureTarget::Left => display_name(&self.cfg.left),
            CaptureTarget::Right => display_name(&self.cfg.right),
            CaptureTarget::Fast => {
                let names: Vec<String> = self.cfg.fast_keys.iter().map(|k| display_name(k)).collect();
                names.join(" + ")
            }
            CaptureTarget::Slow => {
                let names: Vec<String> = self.cfg.slow_keys.iter().map(|k| display_name(k)).collect();
                names.join(" + ")
            }
            CaptureTarget::LeftClick => display_name(&self.cfg.left_click),
            CaptureTarget::RightClick => display_name(&self.cfg.right_click),
            CaptureTarget::MiddleClick => display_name(&self.cfg.middle_click),
            CaptureTarget::ScrollUp => display_name(&self.cfg.scroll_up),
            CaptureTarget::ScrollDown => display_name(&self.cfg.scroll_down),
            CaptureTarget::Cancel => display_name(&self.cfg.cancel),
        }
    }

    fn set_binding(&mut self, target: CaptureTarget, keysym: &str) {
        match target {
            CaptureTarget::Modifier => self.cfg.modifier = keysym.to_string(),
            CaptureTarget::Up => self.cfg.up = keysym.to_string(),
            CaptureTarget::Down => self.cfg.down = keysym.to_string(),
            CaptureTarget::Left => self.cfg.left = keysym.to_string(),
            CaptureTarget::Right => self.cfg.right = keysym.to_string(),
            CaptureTarget::Fast => {
                let mut keys = vec![keysym.to_string()];
                if let Some(m) = mirror_keysym(keysym) {
                    keys.push(m.to_string());
                }
                self.cfg.fast_keys = keys;
            }
            CaptureTarget::Slow => {
                let mut keys = vec![keysym.to_string()];
                if let Some(m) = mirror_keysym(keysym) {
                    keys.push(m.to_string());
                }
                self.cfg.slow_keys = keys;
            }
            CaptureTarget::LeftClick => self.cfg.left_click = keysym.to_string(),
            CaptureTarget::RightClick => self.cfg.right_click = keysym.to_string(),
            CaptureTarget::MiddleClick => self.cfg.middle_click = keysym.to_string(),
            CaptureTarget::ScrollUp => self.cfg.scroll_up = keysym.to_string(),
            CaptureTarget::ScrollDown => self.cfg.scroll_down = keysym.to_string(),
            CaptureTarget::Cancel => self.cfg.cancel = keysym.to_string(),
        }
    }

    fn start_capture(&mut self, target: CaptureTarget) {
        match CaptureConn::open() {
            Ok(c) => {
                self.capture = Some(c);
                self.capturing = Some(target);
                self.message = format!("Press a key for «{}»… (Esc cancels)", target.label());
            }
            Err(e) => {
                self.message = format!("Could not capture a key: {e}");
            }
        }
    }

    fn stop_capture(&mut self) {
        self.capture = None;
        self.capturing = None;
    }

    fn poll_capture(&mut self) {
        if self.capturing.is_none() {
            return;
        }
        let Some(capture) = &self.capture else {
            return;
        };
        match capture.poll_key() {
            Some(Some(name)) if !name.is_empty() => {
                let target = self.capturing.unwrap();
                self.set_binding(target, &name);
                self.message = format!("«{}» is now {}", target.label(), display_name(&name));
                self.stop_capture();
            }
            Some(Some(_)) => {
                // Escape: cancel.
                self.message = "Capture cancelled.".to_string();
                self.stop_capture();
            }
            Some(None) => {
                // Unknown key: keep waiting.
            }
            None => {}
        }
    }

    fn on_apply(&mut self) {
        match save(&config_path(), &self.cfg) {
            Ok(()) => {
                daemon_stop();
                daemon_start();
                std::thread::sleep(Duration::from_millis(600));
                self.refresh_status();
                if self.daemon_running {
                    self.message = "Settings applied and the daemon restarted.".to_string();
                } else {
                    self.message = "Settings saved, but the daemon did not start. See ~/.local/share/keymouse/keymouse.log".to_string();
                }
            }
            Err(e) => {
                self.message = format!("Could not save settings: {e}");
            }
        }
    }

    fn on_reset(&mut self) {
        self.cfg = Config::default();
        self.message = "Reset to defaults — press Apply & Restart to use them.".to_string();
    }
}

fn visuals() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    let c = |rgb: (u8, u8, u8)| egui::Color32::from_rgb(rgb.0, rgb.1, rgb.2);
    v.window_fill = c(BG);
    v.panel_fill = c(PANEL);
    v.extreme_bg_color = c(BG);
    v.faint_bg_color = c(PANEL);
    v.widgets.noninteractive.bg_fill = c(PANEL);
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, c(FG));
    v.widgets.inactive.bg_fill = egui::Color32::from_rgb(0x3a, 0x41, 0x52);
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, c(FG));
    v.widgets.hovered.bg_fill = egui::Color32::from_rgb(0x3a, 0x41, 0x52);
    v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, c(ACCENT));
    v.widgets.active.bg_fill = c(ACCENT);
    v.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(0x06, 0x25, 0x1f));
    v.selection.bg_fill = c(ACCENT);
    v.selection.stroke = egui::Stroke::new(1.0_f32, c(ACCENT));
    v
}

impl eframe::App for SettingsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(visuals());
        // Poll the status every 2 s.
        if self.last_status_check.elapsed() >= Duration::from_secs(2) {
            self.refresh_status();
            self.last_status_check = Instant::now();
        }
        // Key capture runs while capturing: keep repainting.
        if self.capturing.is_some() {
            self.poll_capture();
            ctx.request_repaint();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("KeyMouse");
                    ui.colored_label(
                        egui::Color32::from_rgb(ACCENT.0, ACCENT.1, ACCENT.2),
                        egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).strong().small(),
                    );
                });
                ui.label("موس با کیبورد — نگه دار و با جهت‌ها حرکت بده");
                ui.add_space(8.0);

                // ---- status ------------------------------------------------
                ui.group(|ui| {
                    ui.label(egui::RichText::new("Status").strong().color(egui::Color32::from_rgb(ACCENT.0, ACCENT.1, ACCENT.2)));
                    ui.horizontal(|ui| {
                        let (dot, text) = if self.daemon_running {
                            ("●", "Running — ready to move the pointer")
                        } else {
                            ("●", "Stopped — press Start")
                        };
                        let col = if self.daemon_running { OK } else { DANGER };
                        ui.label(egui::RichText::new(dot).color(egui::Color32::from_rgb(col.0, col.1, col.2)).size(18.0));
                        ui.label(text);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Stop").clicked() {
                                daemon_stop();
                                self.refresh_status();
                            }
                            if ui.button("Start").clicked() {
                                daemon_start();
                                std::thread::sleep(Duration::from_millis(500));
                                self.refresh_status();
                            }
                        });
                    });
                    let mut autostart = self.autostart;
                    if ui.checkbox(&mut autostart, "Start automatically on login (Autostart)").changed() {
                        set_autostart(autostart);
                        self.autostart = autostart;
                    }
                });
                ui.add_space(8.0);

                // ---- keybindings -------------------------------------------
                ui.group(|ui| {
                    ui.label(egui::RichText::new("Keybindings — click a row, then press a key").strong().color(egui::Color32::from_rgb(ACCENT.0, ACCENT.1, ACCENT.2)));
                    ui.label(egui::RichText::new("Click a binding, press the new key, done. Esc cancels.").color(egui::Color32::from_rgb(MUTED.0, MUTED.1, MUTED.2)).small());
                    for target in [
                        CaptureTarget::Modifier,
                        CaptureTarget::Up,
                        CaptureTarget::Down,
                        CaptureTarget::Left,
                        CaptureTarget::Right,
                        CaptureTarget::Fast,
                        CaptureTarget::Slow,
                        CaptureTarget::LeftClick,
                        CaptureTarget::RightClick,
                        CaptureTarget::MiddleClick,
                        CaptureTarget::ScrollUp,
                        CaptureTarget::ScrollDown,
                        CaptureTarget::Cancel,
                    ] {
                        let capturing_this = self.capturing == Some(target);
                        ui.horizontal(|ui| {
                            ui.label(target.label());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let text = if capturing_this {
                                    "… press a key …".to_string()
                                } else {
                                    self.binding_value(target)
                                };
                                let btn = ui.button(egui::RichText::new(text).strong());
                                if btn.clicked() && !capturing_this {
                                    self.start_capture(target);
                                }
                            });
                        });
                    }
                });
                ui.add_space(8.0);

                // ---- pointer speed ------------------------------------------
                ui.group(|ui| {
                    ui.label(egui::RichText::new("Pointer speed").strong().color(egui::Color32::from_rgb(ACCENT.0, ACCENT.1, ACCENT.2)));
                    let mut base = self.cfg.base_speed;
                    if ui.add(egui::Slider::new(&mut base, 1.0..=20.0).text("Start speed (px per tick)").step_by(1.0)).changed() {
                        self.cfg.base_speed = base;
                    }
                    let mut max = self.cfg.max_speed;
                    if ui.add(egui::Slider::new(&mut max, 10.0..=200.0).text("Max speed (px per tick)").step_by(1.0)).changed() {
                        self.cfg.max_speed = max;
                    }
                    let mut accel = self.cfg.accel_per_sec;
                    if ui.add(egui::Slider::new(&mut accel, 0.0..=500.0).text("Acceleration (px per second)").step_by(5.0)).changed() {
                        self.cfg.accel_per_sec = accel;
                    }
                    let mut fast = self.cfg.fast_multiplier;
                    if ui.add(egui::Slider::new(&mut fast, 1.0..=5.0).text("Fast multiplier (×)").step_by(0.1)).changed() {
                        self.cfg.fast_multiplier = fast;
                    }
                    let mut slow = self.cfg.slow_multiplier;
                    if ui.add(egui::Slider::new(&mut slow, 0.05..=1.0).text("Precise multiplier (×)").step_by(0.05)).changed() {
                        self.cfg.slow_multiplier = slow;
                    }
                    let normal = self.cfg.px_per_sec(1.0);
                    let fast_v = self.cfg.px_per_sec(self.cfg.fast_multiplier);
                    let slow_v = self.cfg.px_per_sec(self.cfg.slow_multiplier);
                    ui.label(
                        egui::RichText::new(format!(
                            "Max speed: normal {normal:.0} px/s  •  fast {fast_v:.0}  •  precise {slow_v:.0}"
                        ))
                        .color(egui::Color32::from_rgb(MUTED.0, MUTED.1, MUTED.2)),
                    );
                });
                ui.add_space(8.0);

                // ---- behaviour ----------------------------------------------
                ui.group(|ui| {
                    ui.label(egui::RichText::new("Behaviour").strong().color(egui::Color32::from_rgb(ACCENT.0, ACCENT.1, ACCENT.2)));
                    let mut tick = self.cfg.tick_ms as f64;
                    if ui.add(egui::Slider::new(&mut tick, 5.0..=100.0).text("Tick (ms)").step_by(1.0)).changed() {
                        self.cfg.tick_ms = tick as u64;
                    }
                    let mut scroll = self.cfg.scroll_ms as f64;
                    if ui.add(egui::Slider::new(&mut scroll, 30.0..=1000.0).text("Scroll repeat (ms)").step_by(10.0)).changed() {
                        self.cfg.scroll_ms = scroll as u64;
                    }
                });
                ui.add_space(8.0);

                // ---- actions -------------------------------------------------
                ui.horizontal(|ui| {
                    if ui.button(egui::RichText::new("Apply & Restart").strong()).clicked() {
                        self.on_apply();
                    }
                    if ui.button("Reset to defaults").clicked() {
                        self.on_reset();
                    }
                });
                if !self.message.is_empty() {
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(&self.message).color(egui::Color32::from_rgb(MUTED.0, MUTED.1, MUTED.2)));
                }
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(format!("keymouse v{} — Rust | Config: ~/.config/keymouse/config.toml", env!("CARGO_PKG_VERSION")))
                        .small()
                        .color(egui::Color32::from_rgb(MUTED.0, MUTED.1, MUTED.2)),
                );
            });
        });

        // Smoke-test hook: after a few real frames, save + close.
        if self.smoke_test {
            self.frames += 1;
            if self.frames >= 3 {
                let _ = save(&config_path(), &self.cfg);
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
}

fn main() -> eframe::Result<()> {
    // Hidden test hook: --smoke-test renders a few frames, saves the
    // config and exits (used by the Xvfb GUI smoke test).
    let smoke_test = std::env::args().any(|a| a == "--smoke-test");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([560.0, 800.0])
            .with_title("KeyMouse — Settings"),
        ..Default::default()
    };
    eframe::run_native(
        "KeyMouse Settings",
        options,
        Box::new(move |_cc| Ok(Box::new(SettingsApp::new(smoke_test)))),
    )
}
