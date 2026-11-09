//! Configuration: TOML at ~/.config/keymouse/config.toml (v2).
//! The old v1 .ini (~/.config/keymouse/config.ini) is migrated
//! automatically on first load, then the TOML file is used from then on.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Where the v2 config lives.
pub fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".config").join("keymouse").join("config.toml")
}

/// Where the v1 config lived (migrated from, never written to).
pub fn legacy_config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".config").join("keymouse").join("config.ini")
}

/// Everything the daemon and the settings UI can change.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Keysym name of the key you HOLD (e.g. "Control_R").
    pub modifier: String,
    /// Movement keys (keysym names).
    pub up: String,
    pub down: String,
    pub left: String,
    pub right: String,
    /// Fast keys (any one held = fast). Default: both Shifts.
    pub fast_keys: Vec<String>,
    /// Slow/precise keys (any one held = slow). Default: both Alts.
    pub slow_keys: Vec<String>,
    /// Click keys (keysym names, pressed while the modifier is held).
    pub left_click: String,
    pub right_click: String,
    pub middle_click: String,
    /// Scroll keys (held = repeat scroll).
    pub scroll_up: String,
    pub scroll_down: String,
    /// Cancel key (releases everything).
    pub cancel: String,
    /// Pointer speed, px per tick.
    pub base_speed: f64,
    pub max_speed: f64,
    pub accel_per_sec: f64,
    /// Main loop tick, milliseconds.
    pub tick_ms: u64,
    pub fast_multiplier: f64,
    pub slow_multiplier: f64,
    /// Hold-to-scroll repeat interval, milliseconds.
    pub scroll_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            modifier: "Control_R".to_string(),
            up: "Up".to_string(),
            down: "Down".to_string(),
            left: "Left".to_string(),
            right: "Right".to_string(),
            fast_keys: vec!["Shift_L".to_string(), "Shift_R".to_string()],
            slow_keys: vec!["Alt_L".to_string(), "Alt_R".to_string()],
            left_click: "Return".to_string(),
            right_click: "period".to_string(),
            middle_click: "slash".to_string(),
            scroll_up: "Prior".to_string(),
            scroll_down: "Next".to_string(),
            cancel: "Escape".to_string(),
            base_speed: 6.0,
            max_speed: 42.0,
            accel_per_sec: 90.0,
            tick_ms: 16,
            fast_multiplier: 3.0,
            slow_multiplier: 0.25,
            scroll_ms: 120,
        }
    }
}

impl Config {
    /// Clamp every numeric field into its sane range (a hand-edited
    /// config must never crash the daemon).
    pub fn sanitize(&mut self) {
        let clamp = |v: f64, lo: f64, hi: f64, dflt: f64| -> f64 {
            if v.is_finite() {
                v.clamp(lo, hi)
            } else {
                dflt
            }
        };
        self.base_speed = clamp(self.base_speed, 1.0, 50.0, 6.0);
        self.max_speed = clamp(self.max_speed, self.base_speed, 200.0, 42.0);
        self.accel_per_sec = clamp(self.accel_per_sec, 0.0, 500.0, 90.0);
        self.tick_ms = self.tick_ms.clamp(5, 100);
        self.fast_multiplier = clamp(self.fast_multiplier, 1.0, 5.0, 3.0);
        self.slow_multiplier = clamp(self.slow_multiplier, 0.05, 1.0, 0.25);
        self.scroll_ms = self.scroll_ms.clamp(30, 1000);
        for key in [
            &mut self.modifier,
            &mut self.up,
            &mut self.down,
            &mut self.left,
            &mut self.right,
            &mut self.left_click,
            &mut self.right_click,
            &mut self.middle_click,
            &mut self.scroll_up,
            &mut self.scroll_down,
            &mut self.cancel,
        ] {
            if key.trim().is_empty() {
                *key = Config::default().modifier.clone();
            }
            *key = key.trim().to_string();
        }
        if self.fast_keys.is_empty() {
            self.fast_keys = vec!["Shift_L".to_string(), "Shift_R".to_string()];
        }
        if self.slow_keys.is_empty() {
            self.slow_keys = vec!["Alt_L".to_string(), "Alt_R".to_string()];
        }
    }

    /// Load from `path` (or the default location). Missing/bad values
    /// fall back to defaults; a v1 .ini is migrated if present.
    pub fn load(path: Option<&std::path::Path>) -> Config {
        let path = path
            .map(|p| p.to_path_buf())
            .unwrap_or_else(config_path);
        if path.exists() {
            if let Ok(text) = std::fs::read_to_string(&path) {
                if let Ok(mut cfg) = toml::from_str::<Config>(&text) {
                    cfg.sanitize();
                    return cfg;
                }
                eprintln!("keymouse: WARNING: bad config {path:?}; using defaults");
            }
            return Config::default();
        }
        // No TOML yet: migrate the v1 .ini if it exists.
        let legacy = legacy_config_path();
        if legacy.exists() {
            if let Some(cfg) = migrate_ini(&legacy) {
                let mut cfg = cfg;
                cfg.sanitize();
                let _ = save(&path, &cfg); // keep the migrated copy
                return cfg;
            }
        }
        Config::default()
    }

    /// Max pointer speed in px/sec at the given multiplier (for the GUI
    /// live preview).
    pub fn px_per_sec(&self, multiplier: f64) -> f64 {
        let tick_s = self.tick_ms as f64 / 1000.0;
        self.max_speed / tick_s * multiplier
    }
}

/// Write the config as TOML (creates the parent dir if needed).
pub fn save(path: &std::path::Path, cfg: &Config) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = toml::to_string_pretty(cfg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    let header = "# keymouse config (v2, TOML) — edit and restart keymouse,\n\
                  # or change it in the settings window (keymouse-settings).\n\
                  # Keys are X11 keysym names, e.g. Control_R, Up, Return.\n\n";
    std::fs::write(path, format!("{header}{text}"))
}

/// Parse a v1 .ini file ([mouse] section, key = value) into a v2 Config.
/// Returns None if the file cannot be read at all.
fn migrate_ini(path: &std::path::Path) -> Option<Config> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut cfg = Config::default();
    let mut in_mouse = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_mouse = line == "[mouse]";
            continue;
        }
        if !in_mouse || line.starts_with('#') || line.starts_with(';') || line.is_empty() {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let (k, v) = (k.trim(), v.trim());
        let parse_f = |s: &str| s.parse::<f64>().ok();
        let parse_u = |s: &str| s.parse::<u64>().ok();
        match k {
            "modifier" => cfg.modifier = v.to_string(),
            "base_speed" => {
                if let Some(x) = parse_f(v) {
                    cfg.base_speed = x;
                }
            }
            "max_speed" => {
                if let Some(x) = parse_f(v) {
                    cfg.max_speed = x;
                }
            }
            "accel_per_sec" => {
                if let Some(x) = parse_f(v) {
                    cfg.accel_per_sec = x;
                }
            }
            "tick_ms" => {
                if let Some(x) = parse_u(v) {
                    cfg.tick_ms = x;
                }
            }
            "fast_multiplier" => {
                if let Some(x) = parse_f(v) {
                    cfg.fast_multiplier = x;
                }
            }
            "slow_multiplier" => {
                if let Some(x) = parse_f(v) {
                    cfg.slow_multiplier = x;
                }
            }
            "scroll_ms" => {
                if let Some(x) = parse_u(v) {
                    cfg.scroll_ms = x;
                }
            }
            "left_click_key" => cfg.left_click = v.to_string(),
            "right_click_key" => cfg.right_click = v.to_string(),
            "middle_click_key" => cfg.middle_click = v.to_string(),
            _ => {}
        }
    }
    Some(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn defaults_match_v1() {
        let c = Config::default();
        assert_eq!(c.modifier, "Control_R");
        assert_eq!(c.left_click, "Return");
        assert_eq!(c.base_speed, 6.0);
        assert_eq!(c.max_speed, 42.0);
        assert_eq!(c.fast_keys, vec!["Shift_L", "Shift_R"]);
        assert_eq!(c.slow_keys, vec!["Alt_L", "Alt_R"]);
        assert_eq!(c.scroll_up, "Prior");
        assert_eq!(c.cancel, "Escape");
    }

    #[test]
    fn sanitize_clamps_bad_values() {
        let mut c = Config::default();
        c.base_speed = f64::NAN;
        c.max_speed = 5.0; // below base -> clamped up to base
        c.tick_ms = 0;
        c.fast_multiplier = 99.0;
        c.sanitize();
        assert_eq!(c.base_speed, 6.0);
        assert!(c.max_speed >= c.base_speed);
        assert_eq!(c.tick_ms, 5);
        assert_eq!(c.fast_multiplier, 5.0);
    }

    #[test]
    fn toml_round_trip() {
        let dir = std::env::temp_dir().join("keymouse-test-cfg");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("config.toml");
        let mut c = Config::default();
        c.modifier = "Caps_Lock".to_string();
        c.base_speed = 8.0;
        save(&path, &c).unwrap();
        let back = Config::load(Some(&path));
        assert_eq!(back.modifier, "Caps_Lock");
        assert_eq!(back.base_speed, 8.0);
        assert_eq!(back.max_speed, 42.0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bad_toml_falls_back_to_defaults() {
        let dir = std::env::temp_dir().join("keymouse-test-bad");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("config.toml");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(b"this is [ not valid toml").unwrap();
        }
        let c = Config::load(Some(&path));
        assert_eq!(c.modifier, "Control_R");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ini_migration() {
        let dir = std::env::temp_dir().join("keymouse-test-ini");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("config.ini");
        std::fs::write(
            &path,
            "[mouse]\nmodifier = Caps_Lock\nbase_speed = banana\nmax_speed = 5\nleft_click_key = space\n",
        )
        .unwrap();
        let c = migrate_ini(&path).unwrap();
        assert_eq!(c.modifier, "Caps_Lock");
        assert_eq!(c.left_click, "space");
        // banana -> default after sanitize; max 5 < base -> clamped
        let mut c = c;
        c.sanitize();
        assert_eq!(c.base_speed, 6.0);
        assert!(c.max_speed >= c.base_speed);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn px_per_sec_preview() {
        let c = Config::default();
        // max 42 px/tick at 16ms tick = 2625 px/s normal, x3 fast.
        assert!((c.px_per_sec(1.0) - 2625.0).abs() < 1.0);
        assert!((c.px_per_sec(3.0) - 7875.0).abs() < 1.0);
    }
}
