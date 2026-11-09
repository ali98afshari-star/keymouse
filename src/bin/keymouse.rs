//! keymouse daemon (v2, Rust).
//!
//! Hold ONE modifier key (default Right Ctrl) and move the mouse with
//! the arrow keys. While the modifier is held the keyboard is grabbed,
//! so the arrows never reach the app underneath.
//!
//! Behaviour is identical to the v1.1 Python daemon:
//! Shift = fast x3, Alt = slow x0.25 (slow wins), Enter = left
//! button (hold to drag), period = right click, slash = middle click,
//! PageUp/PageDown = hold-to-scroll, Esc = cancel, release of the
//! modifier releases any held button (drag safety).
//!
//! Usage: keymouse [--config PATH] [--test] [--once]

use keymouse::config::{config_path, Config};
use keymouse::logic::{direction_vector, move_step, HeldDirs};
use keymouse::x11util::{keycode_for_name, xtest_available, Keymap};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::xtest;
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::NONE;

// X event type codes (core protocol).
const BUTTON_PRESS: u8 = 4;
const BUTTON_RELEASE: u8 = 5;

/// What a grabbed key does while the modifier is held.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Action {
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

/// Shared daemon state (main event thread <-> mover thread).
#[derive(Default)]
struct State {
    active: bool,
    held_dirs: HeldDirs,
    held_start: Option<Instant>,
    fast: bool,
    slow: bool,
    scroll_up: bool,
    scroll_down: bool,
    /// None = scroll immediately on the next mover tick.
    last_scroll: Option<Instant>,
    left_down: bool,
    pressed_clicks: HashSet<Action>,
}

/// Everything the mover thread needs.
struct MoverCtx {
    state: Arc<Mutex<State>>,
    action_conn: Arc<Mutex<RustConnection>>,
    stop: Arc<AtomicBool>,
    cfg: Config,
    xtest_ok: bool,
}

fn mover(ctx: MoverCtx) {
    let tick = Duration::from_millis(ctx.cfg.tick_ms);
    loop {
        if ctx.stop.load(Ordering::Relaxed) {
            break;
        }
        // Snapshot the state (never hold the lock while doing X calls).
        let (active, held, held_start, fast, slow, scroll_up, scroll_down, last_scroll) = {
            let st = ctx.state.lock().unwrap();
            (
                st.active,
                st.held_dirs,
                st.held_start,
                st.fast,
                st.slow,
                st.scroll_up,
                st.scroll_down,
                st.last_scroll,
            )
        };
        if active {
            if !held.is_empty() {
                let held_s = held_start.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0);
                let (dx, dy) = move_step(
                    held,
                    held_s,
                    fast,
                    slow,
                    ctx.cfg.base_speed,
                    ctx.cfg.max_speed,
                    ctx.cfg.accel_per_sec,
                    ctx.cfg.fast_multiplier,
                    ctx.cfg.slow_multiplier,
                );
                if dx != 0 || dy != 0 {
                    if let Ok(conn) = ctx.action_conn.lock() {
                        // Both windows None => move relative to current position.
                        let _ = conn.warp_pointer(NONE, NONE, 0, 0, 0, 0, dx as i16, dy as i16);
                        let _ = conn.flush();
                    }
                }
            }
            if scroll_up || scroll_down {
                let now = Instant::now();
                let due = match last_scroll {
                    None => true,
                    Some(t) => now.duration_since(t) >= Duration::from_millis(ctx.cfg.scroll_ms),
                };
                if due && ctx.xtest_ok {
                    if let Ok(conn) = ctx.action_conn.lock() {
                        if scroll_up {
                            scroll_once(&conn, 4);
                        }
                        if scroll_down {
                            scroll_once(&conn, 5);
                        }
                    }
                    let mut st = ctx.state.lock().unwrap();
                    st.last_scroll = Some(Instant::now());
                }
            }
        }
        std::thread::sleep(tick);
    }
}

/// One scroll step: press + release of button 4 (up) or 5 (down).
fn scroll_once(conn: &RustConnection, button: u8) {
    let _ = xtest::fake_input(conn, BUTTON_PRESS, button, 0, 0, 0, 0, 0);
    let _ = xtest::fake_input(conn, BUTTON_RELEASE, button, 0, 0, 0, 0, 0);
    let _ = conn.flush();
}

fn click(conn: &RustConnection, button: u8, press: bool) {
    let ty = if press { BUTTON_PRESS } else { BUTTON_RELEASE };
    let _ = xtest::fake_input(conn, ty, button, 0, 0, 0, 0, 0);
    let _ = conn.flush();
}

struct Daemon {
    /// keycode -> action (first wins; conflicts are warned about).
    code_to_action: HashMap<u8, Action>,
    modifier_kc: u8,
    state: Arc<Mutex<State>>,
    action_conn: Arc<Mutex<RustConnection>>,
    xtest_ok: bool,
}

impl Daemon {
    fn new(
        cfg: Config,
        keymap: &Keymap,
        action_conn: Arc<Mutex<RustConnection>>,
    ) -> Result<Daemon, String> {
        // Resolve every configured keysym to a keycode.
        let mut pairs: Vec<(Action, String)> = vec![
            (Action::Modifier, cfg.modifier.clone()),
            (Action::Up, cfg.up.clone()),
            (Action::Down, cfg.down.clone()),
            (Action::Left, cfg.left.clone()),
            (Action::Right, cfg.right.clone()),
            (Action::LeftClick, cfg.left_click.clone()),
            (Action::RightClick, cfg.right_click.clone()),
            (Action::MiddleClick, cfg.middle_click.clone()),
            (Action::ScrollUp, cfg.scroll_up.clone()),
            (Action::ScrollDown, cfg.scroll_down.clone()),
            (Action::Cancel, cfg.cancel.clone()),
        ];
        for k in &cfg.fast_keys {
            pairs.push((Action::Fast, k.clone()));
        }
        for k in &cfg.slow_keys {
            pairs.push((Action::Slow, k.clone()));
        }
        let mut code_to_action: HashMap<u8, Action> = HashMap::new();
        let mut modifier_kc: Option<u8> = None;
        for (action, name) in pairs {
            match keycode_for_name(keymap, &name) {
                Some(kc) => {
                    if action == Action::Modifier {
                        modifier_kc = Some(kc);
                    }
                    if let Some(prev) = code_to_action.get(&kc) {
                        eprintln!(
                            "keymouse: WARNING: key {name:?} (keycode {kc}) already bound to {prev:?}; ignoring"
                        );
                    } else {
                        code_to_action.insert(kc, action);
                    }
                }
                None => {
                    eprintln!(
                        "keymouse: WARNING: no keycode for keysym {name:?} ({action:?}), that key will not work"
                    );
                }
            }
        }
        let modifier_kc = modifier_kc.ok_or_else(|| {
            format!(
                "keymouse: ERROR: modifier keysym {:?} has no keycode on this keyboard; fix 'modifier' in the config and restart",
                cfg.modifier
            )
        })?;
        Ok(Daemon {
            code_to_action,
            modifier_kc,
            state: Arc::new(Mutex::new(State::default())),
            action_conn,
            xtest_ok: true, // set by caller
        })
    }

    fn activate(&self) {
        let mut st = self.state.lock().unwrap();
        st.active = true;
        st.held_dirs = HeldDirs::default();
        st.held_start = None;
        st.fast = false;
        st.slow = false;
        st.scroll_up = false;
        st.scroll_down = false;
        st.pressed_clicks.clear();
        st.last_scroll = None;
        println!(
            "keymouse: active — arrows move the pointer, release the modifier to stop"
        );
    }

    fn deactivate(&self) {
        let left_down = {
            let mut st = self.state.lock().unwrap();
            if !st.active {
                return;
            }
            st.active = false;
            let ld = st.left_down;
            st.held_dirs = HeldDirs::default();
            st.fast = false;
            st.slow = false;
            st.scroll_up = false;
            st.scroll_down = false;
            st.pressed_clicks.clear();
            st.left_down = false;
            ld
        };
        if left_down {
            // Safety: never leave a drag stuck down.
            if let Ok(conn) = self.action_conn.lock() {
                click(&conn, 1, false);
            }
        }
    }

    fn on_key(&self, keycode: u8, is_press: bool) {
        let Some(action) = self.code_to_action.get(&keycode).copied() else {
            return;
        };
        if action == Action::Modifier {
            if !is_press {
                self.deactivate();
            }
            return;
        }
        let mut st = self.state.lock().unwrap();
        match action {
            Action::Up | Action::Down | Action::Left | Action::Right => {
                if is_press {
                    if st.held_dirs.is_empty() {
                        st.held_start = Some(Instant::now());
                    }
                    match action {
                        Action::Up => st.held_dirs.up = true,
                        Action::Down => st.held_dirs.down = true,
                        Action::Left => st.held_dirs.left = true,
                        _ => st.held_dirs.right = true,
                    }
                } else {
                    match action {
                        Action::Up => st.held_dirs.up = false,
                        Action::Down => st.held_dirs.down = false,
                        Action::Left => st.held_dirs.left = false,
                        _ => st.held_dirs.right = false,
                    }
                    if st.held_dirs.is_empty() {
                        st.held_start = None;
                    }
                }
            }
            Action::Fast => st.fast = is_press,
            Action::Slow => st.slow = is_press,
            Action::LeftClick => {
                if is_press {
                    if st.pressed_clicks.insert(Action::LeftClick) {
                        st.left_down = true;
                        let xtest_ok = self.xtest_ok;
                        drop(st);
                        if xtest_ok {
                            if let Ok(conn) = self.action_conn.lock() {
                                click(&conn, 1, true);
                            }
                        }
                    }
                } else {
                    st.pressed_clicks.remove(&Action::LeftClick);
                    st.left_down = false;
                    let xtest_ok = self.xtest_ok;
                    drop(st);
                    if xtest_ok {
                        if let Ok(conn) = self.action_conn.lock() {
                            click(&conn, 1, false);
                        }
                    }
                }
            }
            Action::RightClick | Action::MiddleClick => {
                let button = if action == Action::RightClick { 3 } else { 2 };
                if is_press {
                    if st.pressed_clicks.insert(action) {
                        let xtest_ok = self.xtest_ok;
                        drop(st);
                        if xtest_ok {
                            if let Ok(conn) = self.action_conn.lock() {
                                click(&conn, button, true);
                                click(&conn, button, false);
                            }
                        }
                    }
                } else {
                    st.pressed_clicks.remove(&action);
                }
            }
            Action::ScrollUp | Action::ScrollDown => {
                let slot = if action == Action::ScrollUp {
                    &mut st.scroll_up
                } else {
                    &mut st.scroll_down
                };
                if is_press {
                    *slot = true;
                    st.last_scroll = None; // scroll once immediately
                } else {
                    *slot = false;
                }
            }
            Action::Cancel => {
                if is_press {
                    drop(st);
                    self.deactivate();
                }
            }
            Action::Modifier => unreachable!(),
        }
    }
}

fn single_instance() -> Result<std::fs::File, String> {
    use fs2::FileExt;
    let dir = keymouse::data_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let lock_path = dir.join("keymouse.lock");
    let fh = std::fs::File::create(&lock_path).map_err(|e| e.to_string())?;
    fh.try_lock_exclusive().map_err(|_| {
        "keymouse: already running — refusing a second copy (kill the old one first: pkill -f keymouse)".to_string()
    })?;
    Ok(fh) // keep referenced: the lock dies with this process
}

fn self_test() -> std::process::ExitCode {
    // Pure-logic checks, no X needed. Mirrors the v1.1 --test.
    let cfg = Config::default();
    assert_eq!(cfg.modifier, "Control_R");
    assert_eq!(cfg.left_click, "Return");
    let speed = |s: f64| keymouse::logic::speed_at(s, cfg.base_speed, cfg.max_speed, cfg.accel_per_sec);
    assert_eq!(speed(0.0), 6.0);
    assert_eq!(speed(10.0), 42.0);
    assert_eq!(direction_vector(HeldDirs { up: true, ..Default::default() }), (0.0, -1.0));
    assert!(direction_vector(HeldDirs { left: true, down: true, ..Default::default() }).0 < 0.0);
    let d = direction_vector(HeldDirs { up: true, right: true, ..Default::default() });
    assert!(((d.0 * d.0 + d.1 * d.1).sqrt() - 1.0).abs() < 1e-9, "diagonal {:?}", d);
    let step = |h: HeldDirs, s: f64, f: bool, sl: bool| {
        move_step(h, s, f, sl, cfg.base_speed, cfg.max_speed, cfg.accel_per_sec, cfg.fast_multiplier, cfg.slow_multiplier)
    };
    assert_eq!(step(HeldDirs::default(), 0.0, false, false), (0, 0));
    assert_eq!(step(HeldDirs { right: true, ..Default::default() }, 0.0, false, false), (6, 0));
    assert_eq!(step(HeldDirs { right: true, ..Default::default() }, 0.0, true, false), (18, 0));
    assert_eq!(step(HeldDirs { right: true, ..Default::default() }, 0.0, false, true), (2, 0));
    assert_eq!(step(HeldDirs { right: true, ..Default::default() }, 0.0, true, true), (2, 0));
    assert_eq!(step(HeldDirs { up: true, ..Default::default() }, 0.0, false, false), (0, -6));
    println!("keymouse --test: all self-checks passed");
    std::process::ExitCode::SUCCESS
}

fn parse_args(argv: &[String]) -> (Option<std::path::PathBuf>, bool, bool, bool) {
    let mut config: Option<std::path::PathBuf> = None;
    let mut test = false;
    let mut once = false;
    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "--config" => {
                if i + 1 >= argv.len() {
                    eprintln!("keymouse: ERROR: --config needs a path after it");
                    return (None, false, false, false);
                }
                config = Some(std::path::PathBuf::from(&argv[i + 1]));
                i += 1;
            }
            "--test" => test = true,
            "--once" => once = true,
            _ => {}
        }
        i += 1;
    }
    (config, test, once, true)
}

fn main() -> std::process::ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let (config_arg, test, once, ok) = parse_args(&argv);
    if !ok {
        return std::process::ExitCode::from(2);
    }
    if test {
        return self_test();
    }
    let cfg = Config::load(config_arg.as_deref());

    // Connect to X (two connections: events + actions).
    let (conn, screen_num) = match x11rb::connect(None) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("keymouse: ERROR: cannot connect to X: {e}");
            return std::process::ExitCode::from(1);
        }
    };
    let (action_conn, _) = match x11rb::connect(None) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("keymouse: ERROR: cannot connect to X: {e}");
            return std::process::ExitCode::from(1);
        }
    };
    let action_conn = Arc::new(Mutex::new(action_conn));
    let xtest_ok = xtest_available(&conn);
    if !xtest_ok {
        eprintln!("keymouse: WARNING: XTest unavailable; clicks/scroll disabled, movement still works");
    }

    if once {
        // Smoke test: move the pointer 1 px and exit.
        let _ = conn.warp_pointer(NONE, NONE, 0, 0, 0, 0, 1, 0);
        let _ = conn.flush();
        println!("keymouse --once: moved +1px ok");
        return std::process::ExitCode::SUCCESS;
    }

    let keymap = match Keymap::fetch(&conn) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("keymouse: ERROR: cannot read keyboard mapping: {e}");
            return std::process::ExitCode::from(1);
        }
    };
    let mut daemon = match Daemon::new(cfg.clone(), &keymap, Arc::clone(&action_conn)) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{e}");
            return std::process::ExitCode::from(1);
        }
    };
    daemon.xtest_ok = xtest_ok;

    // Single-instance lock (held for the life of the process).
    let _lock = match single_instance() {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{e}");
            return std::process::ExitCode::from(1);
        }
    };

    // Mover thread.
    let stop = Arc::new(AtomicBool::new(false));
    let mover_ctx = MoverCtx {
        state: Arc::clone(&daemon.state),
        action_conn: Arc::clone(&action_conn),
        stop: Arc::clone(&stop),
        cfg: cfg.clone(),
        xtest_ok,
    };
    std::thread::spawn(move || mover(mover_ctx));

    // Grab the modifier key on the root window. owner_events=false:
    // while the modifier is held, ALL key events come to the root window.
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;
    if let Err(e) = conn.change_window_attributes(
        root,
        &ChangeWindowAttributesAux::new()
            .event_mask(EventMask::KEY_PRESS | EventMask::KEY_RELEASE),
    ) {
        eprintln!("keymouse: ERROR: cannot set root event mask: {e}");
        return std::process::ExitCode::from(1);
    }
    if let Err(e) = conn.grab_key(
        false,
        root,
        ModMask::ANY,
        daemon.modifier_kc,
        GrabMode::ASYNC,
        GrabMode::ASYNC,
    ) {
        eprintln!("keymouse: ERROR: cannot grab the modifier key: {e}");
        return std::process::ExitCode::from(1);
    }
    // x11rb buffers void requests: flush so the event mask + grab
    // actually reach the server before we start polling for events.
    let _ = conn.flush();
    println!(
        "keymouse: ready — hold {} + arrows to move the mouse (config: {:?})",
        cfg.modifier,
        config_arg.unwrap_or_else(config_path)
    );

    // Signal handling: SIGINT/SIGTERM set the stop flag. The event loop
    // polls, so it notices within a few ms.
    let stop_sig = Arc::clone(&stop);
    let _ = signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&stop_sig));
    let _ = signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&stop_sig));

    // Main event loop (poll so signals are noticed promptly).
    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        match conn.poll_for_event() {
            Ok(Some(event)) => match event {
                Event::KeyPress(ev) => {
                    if ev.detail == daemon.modifier_kc {
                        let active = daemon.state.lock().unwrap().active;
                        if !active {
                            daemon.activate();
                        }
                    } else {
                        let active = daemon.state.lock().unwrap().active;
                        if active {
                            daemon.on_key(ev.detail, true);
                        }
                    }
                }
                Event::KeyRelease(ev) => {
                    let active = daemon.state.lock().unwrap().active;
                    if active {
                        daemon.on_key(ev.detail, false);
                    }
                }
                _ => {}
            },
            Ok(None) => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => {
                eprintln!("keymouse: X connection error: {e}");
                break;
            }
        }
    }

    daemon.deactivate();
    stop.store(true, Ordering::Relaxed);
    std::process::ExitCode::SUCCESS
}
