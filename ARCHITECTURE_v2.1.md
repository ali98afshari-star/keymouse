# KeyMouse v2.1 — Architecture & Refactoring Plan

## Executive Summary

KeyMouse v2.0 is a solid Rust rewrite of the Python v1 daemon. It works well but has architectural limitations that prevent future growth (Wayland, plugins, hot-reload, async). This document outlines the v2.1 refactoring to establish a clean, extensible architecture.

---

## Current State Analysis (v2.0)

### Strengths
- Clean separation: `logic.rs` (pure) vs `x11util.rs` (X11)
- Unit tests for pure logic (`logic.rs`, `config.rs`)
- Integration tests under Xvfb (e2e + GUI smoke)
- Single binary (~861 KB), low memory (~2.6 MB)
- TOML config with v1 migration
- Game-style key capture in GUI

### Architectural Debt
| Area | Issue | Impact |
|------|-------|--------|
| **Daemon monolith** | 587 lines in `keymouse.rs`: event loop, state, X11, actions all mixed | Hard to test, extend, or port to Wayland |
| **Sync-only** | `std::thread` + blocking `poll_for_event()` | No async, can't use modern Rust patterns |
| **Error handling** | `Result<T, String>` everywhere | No error context, hard to handle programmatically |
| **Logging** | `println!`/`eprintln!` | No levels, no structured output, no file rotation |
| **Config** | Load-once, no hot-reload | Must restart daemon for changes |
| **Actions** | Hard-coded `Action` enum in daemon | Can't add custom actions without code changes |
| **X11 coupling** | Daemon directly uses `x11rb` types | Wayland port requires full rewrite |
| **Testing** | Daemon logic not unit-testable (X11 deps) | Only slow Xvfb e2e tests |
| **Single instance** | File lock in `data_dir()` | Works but not abstracted |

---

## v2.1 Architectural Goals

1. **Platform Abstraction** — Clean `Backend` trait (X11 today, Wayland tomorrow)
2. **Async Runtime** — Tokio-based, proper async event loop
3. **Plugin System** — Actions as plugins, not hard-coded enum
4. **Structured Errors** — `thiserror`/`eyre` with context
5. **Structured Logging** — `tracing` with file + stdout, configurable levels
6. **Hot-Reload Config** — Watch config file, apply without restart
7. **Testable Core** — Pure daemon logic separated from platform I/O
8. **Observability** — Metrics, health checks, structured logs

---

## Module Structure (v2.1)

```
keymouse/
├── Cargo.toml                    # Workspace root
├── crates/
│   ├── keymouse-core/            # Pure logic, no platform deps
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── config.rs         # Config types, load/save, validation, migration
│   │   │   ├── action.rs         # Action trait, built-in actions, registry
│   │   │   ├── state.rs          # Daemon state machine (pure)
│   │   │   ├── motion.rs         # Speed, acceleration, direction (from logic.rs)
│   │   │   ├── keysym.rs         # Keysym table, name<->value, display names
│   │   │   └── error.rs          # Core error types
│   │   └── tests/
│   │       └── core_tests.rs     # Pure logic unit tests
│   │
│   ├── keymouse-backend/         # Platform abstraction
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── backend.rs        # Backend trait (grab, warp, click, scroll, events)
│   │   │   ├── keymap.rs         # Keycode<->keysym mapping
│   │   │   ├── x11/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── connection.rs # X11 connection wrapper
│   │   │   │   ├── grab.rs       # Key/keyboard grab logic
│   │   │   │   ├── input.rs      # Warp, fake input (XTest)
│   │   │   │   └── event.rs      # Event parsing -> CoreEvent
│   │   │   └── wayland/          # Stub for future
│   │   │       ├── mod.rs
│   │   │       └── stub.rs
│   │   └── tests/
│   │       └── backend_tests.rs  # Mock backend tests
│   │
│   ├── keymouse-daemon/          # Daemon binary
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── main.rs           # Entry: args, logging, runtime, shutdown
│   │   │   ├── daemon.rs         # Daemon struct: core + backend + config watch
│   │   │   ├── config_watch.rs   # Hot-reload via notify
│   │   │   ├── signal.rs         # SIGINT/SIGTERM handling
│   │   │   └── metrics.rs        # Prometheus/health (optional)
│   │   └── tests/
│   │       └── daemon_tests.rs   # Integration tests with mock backend
│   │
│   ├── keymouse-settings/        # GUI binary (egui)
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── main.rs           # Entry, eframe setup
│   │   │   ├── app.rs            # SettingsApp (UI logic)
│   │   │   ├── key_capture.rs    # X11 key capture (moved from inline)
│   │   │   ├── daemon_control.rs # Start/stop/restart daemon
│   │   │   ├── autostart.rs      # .desktop file management
│   │   │   └── theme.rs          # Dark theme constants
│   │   └── tests/
│   │       └── gui_tests.rs      # Smoke tests
│   │
│   └── keymouse-common/          # Shared utilities (optional)
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── paths.rs          # Config dir, data dir, lock file
│           ├── logging.rs        # tracing setup
│           └── single_instance.rs # Cross-platform single-instance lock
│
├── tests/                        # Workspace-level integration tests
│   ├── common/
│   │   └── mod.rs                # Xvfb, TempHome, fake_key helpers
│   ├── e2e.rs                    # Full daemon e2e (movement, click, scroll)
│   └── gui_smoke.rs              # GUI renders + saves config
│
├── xtask/                        # Build/release automation (optional)
│   ├── Cargo.toml
│   └── src/
│       └── main.rs               # cargo xtask release, cargo xtask deb
│
├── install.sh                    # User installer (unchanged)
├── uninstall.sh                  # Uninstaller (unchanged)
├── keymouse.desktop              # Autostart entry
├── keymouse-settings.desktop     # Menu entry
├── README.md
├── README.fa.md
└── LICENSE
```

---

## Key Design Decisions

### 1. Backend Trait (Platform Abstraction)

```rust
// keymouse-backend/src/backend.rs
use async_trait::async_trait;
use keymouse_core::{CoreEvent, ActionTrigger, KeyCode, MouseButton};
use std::collections::HashMap;

#[async_trait]
pub trait Backend: Send + Sync {
    /// Initialize connection, return keymap for keycode resolution
    async fn connect(&mut self) -> Result<Keymap, BackendError>;

    /// Grab the modifier key globally
    async fn grab_modifier(&mut self, keycode: KeyCode) -> Result<(), BackendError>;

    /// Release the modifier grab
    async fn ungrab_modifier(&mut self) -> Result<(), BackendError>;

    /// Move pointer relatively (dx, dy in pixels)
    async fn warp_pointer(&mut self, dx: i16, dy: i16) -> Result<(), BackendError>;

    /// Click (press + release) a mouse button
    async fn click(&mut self, button: MouseButton, press: bool) -> Result<(), BackendError>;

    /// Scroll (button 4/5 press+release)
    async fn scroll(&mut self, direction: ScrollDirection) -> Result<(), BackendError>;

    /// Poll for next core event (blocking with timeout)
    async fn next_event(&mut self) -> Result<Option<CoreEvent>, BackendError>;

    /// Get display geometry for bounds checking
    fn screen_size(&self) -> (u16, u16);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreEvent {
    KeyPress(KeyCode),
    KeyRelease(KeyCode),
    ModifierPressed,
    ModifierReleased,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyCode(pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton { Left = 1, Middle = 2, Right = 3, ScrollUp = 4, ScrollDown = 5 }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollDirection { Up, Down }
```

### 2. Action Plugin System

```rust
// keymouse-core/src/action.rs
use async_trait::async_trait;
use keymouse_backend::Backend;
use keymouse_core::state::DaemonState;
use std::collections::HashMap;

#[async_trait]
pub trait Action: Send + Sync {
    fn name(&self) -> &'static str;
    fn trigger_key(&self) -> KeyCode;  // Key that activates this action

    /// Called when the trigger key is pressed (while modifier held)
    async fn on_press(&mut self, state: &mut DaemonState, backend: &mut dyn Backend);

    /// Called when the trigger key is released
    async fn on_release(&mut self, state: &mut DaemonState, backend: &mut dyn Backend);

    /// Called every tick while active (for continuous actions like movement/scroll)
    async fn on_tick(&mut self, state: &mut DaemonState, backend: &mut dyn Backend, dt: Duration) {}
}

/// Registry holds all actions, keyed by trigger keycode
pub struct ActionRegistry {
    actions: HashMap<KeyCode, Box<dyn Action>>,
    modifier_keycode: KeyCode,
}

impl ActionRegistry {
    pub fn new(modifier: KeyCode) -> Self { ... }
    pub fn register(&mut self, action: Box<dyn Action>) { ... }
    pub fn handle_key(&mut self, keycode: KeyCode, press: bool, state: &mut DaemonState, backend: &mut dyn Backend) { ... }
    pub fn tick(&mut self, state: &mut DaemonState, backend: &mut dyn Backend, dt: Duration) { ... }
}
```

**Built-in actions** (in `keymouse-core/src/actions/`):
- `MoveAction` — Arrow keys → pointer movement with acceleration
- `ClickAction` — Enter/Period/Slash → left/right/middle click (drag support)
- `ScrollAction` — PageUp/PageDown → hold-to-scroll
- `ModifierAction` — Fast (Shift) / Slow (Alt) multipliers
- `CancelAction` — Escape → release everything

### 3. Pure Daemon State Machine

```rust
// keymouse-core/src/state.rs
use std::time::Instant;

#[derive(Debug, Clone, Default)]
pub struct DaemonState {
    pub active: bool,
    pub held_dirs: HeldDirs,
    pub held_since: Option<Instant>,
    pub fast: bool,
    pub slow: bool,
    pub scroll_up: bool,
    pub scroll_down: bool,
    pub last_scroll: Option<Instant>,
    pub left_dragging: bool,
}

impl DaemonState {
    pub fn activate(&mut self) { ... }
    pub fn deactivate(&mut self) -> bool { ... } // returns was_dragging
    pub fn set_direction(&mut self, dir: Direction, pressed: bool) { ... }
    pub fn set_modifier(&mut self, kind: ModifierKind, pressed: bool) { ... }
    pub fn set_scroll(&mut self, dir: ScrollDirection, pressed: bool) { ... }
}
```

### 4. Config Hot-Reload

```rust
// keymouse-daemon/src/config_watch.rs
use notify::{Watcher, RecursiveMode, EventKind};
use keymouse_core::Config;

pub struct ConfigWatcher {
    rx: tokio::sync::mpsc::Receiver<Config>,
    _watcher: notify::RecommendedWatcher,
}

impl ConfigWatcher {
    pub fn new(path: &Path) -> Result<Self, ConfigWatchError> {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let mut watcher = notify::recommended_watcher(move |res| {
            if let Ok(Event { kind: EventKind::Modify(_), paths, .. }) = res {
                if paths.iter().any(|p| p == path) {
                    if let Ok(cfg) = Config::load(Some(path)) {
                        let _ = tx.blocking_send(cfg);
                    }
                }
            }
        })?;
        watcher.watch(path.parent().unwrap(), RecursiveMode::NonRecursive)?;
        Ok(Self { rx, _watcher: watcher })
    }

    pub async fn next(&mut self) -> Option<Config> { self.rx.recv().await }
}
```

Daemon applies new config atomically on receipt (rebuilds action registry, updates speeds).

---

## Migration Strategy (v2.0 → v2.1)

### Phase 1: Foundation (Week 1-2)
1. Create workspace `Cargo.toml` with 4 crates
2. Extract `keymouse-core` from existing `src/` (logic, config, keysym, motion)
3. Add `thiserror`, `tracing`, `async-trait`, `tokio` dependencies
4. Write core error types, structured logging setup
5. **All existing unit tests must pass**

### Phase 2: Backend Abstraction (Week 2-3)
1. Create `keymouse-backend` crate with `Backend` trait
2. Move `x11util.rs` → `x11/` module, implement `Backend` for X11
3. Extract `Keymap` and keycode resolution
4. Write mock backend for unit testing daemon logic

### Phase 3: Action System (Week 3-4)
1. Define `Action` trait + `ActionRegistry` in `keymouse-core`
2. Implement 5 built-in actions (Move, Click, Scroll, Modifier, Cancel)
3. Remove hard-coded `Action` enum from daemon
4. Daemon becomes: event loop + state + registry + backend

### Phase 4: Async Daemon (Week 4-5)
1. Create `keymouse-daemon` binary crate
2. Rewrite main loop with Tokio: `select!` on events + tick timer + config watch + signals
3. Add config hot-reload via `notify`
4. Add graceful shutdown (SIGINT/SIGTERM)

### Phase 5: GUI Updates (Week 5-6)
1. Update `keymouse-settings` to use new config paths, daemon control
2. Extract key capture into separate module
3. Add theme as separate module
4. Ensure `--smoke-test` still works

### Phase 6: Integration & Polish (Week 6-7)
1. Run full e2e test suite under Xvfb
2. Benchmark: binary size, RAM, startup time vs v2.0
3. Update `install.sh`/`uninstall.sh` if paths change
4. Documentation updates

---

## Task Breakdown

### Crate: keymouse-core

| ID | Task | Description | Est. |
|----|------|-------------|------|
| C1 | Create crate structure | `cargo new --lib keymouse-core`, workspace config | 0.5d |
| C2 | Extract `config.rs` | Move v2.0 config.rs, add `sanitize()`, migration, tests | 1d |
| C3 | Extract `keysym.rs` | Move v2.0 keysym.rs, add `display_name`, `mirror_keysym` | 0.5d |
| C4 | Extract `motion.rs` | Move `logic.rs` → `motion.rs` (speed, direction, move_step) | 0.5d |
| C5 | Create `action.rs` | Define `Action` trait, `ActionRegistry`, `ModifierKind` | 1d |
| C6 | Built-in actions | Implement `MoveAction`, `ClickAction`, `ScrollAction`, `ModifierAction`, `CancelAction` | 2d |
| C7 | Create `state.rs` | Pure `DaemonState` with transitions, no I/O | 1d |
| C8 | Core error types | `thiserror` types for ConfigError, ActionError, StateError | 0.5d |
| C9 | Unit tests | Port all v2.0 tests + new action/registry tests | 1d |

### Crate: keymouse-backend

| ID | Task | Description | Est. |
|----|------|-------------|------|
| B1 | Create crate + trait | `Backend` trait, `CoreEvent`, `KeyCode`, `MouseButton` | 1d |
| B2 | X11 connection | Wrapper around `x11rb::RustConnection` with async methods | 1d |
| B3 | X11 keymap | Port `Keymap` from x11util, async `fetch()` | 0.5d |
| B4 | X11 grab | Key grab/ungrab, keyboard grab for capture | 0.5d |
| B5 | X11 input | Warp pointer, XTest click, scroll | 0.5d |
| B6 | X11 events | Poll X events → `CoreEvent` stream | 1d |
| B7 | Mock backend | In-memory backend for unit testing daemon | 1d |
| B8 | Backend tests | Test X11 backend under Xvfb, mock backend unit tests | 1d |

### Crate: keymouse-daemon

| ID | Task | Description | Est. |
|----|------|-------------|------|
| D1 | Create binary crate | `keymouse-daemon` with Tokio, tracing, clap | 0.5d |
| D2 | Daemon struct | Holds `Backend`, `ActionRegistry`, `DaemonState`, `ConfigWatcher` | 1d |
| D3 | Async event loop | `select!` on: backend events, tick timer, config watch, shutdown signal | 2d |
| D4 | Config hot-reload | On config change: rebuild registry, update state params | 1d |
| D5 | Signal handling | SIGINT/SIGTERM → graceful stop, release grabs | 0.5d |
| D6 | Single instance | Cross-platform lock file (reuse v2.0 logic) | 0.5d |
| D7 | Logging setup | `tracing` + `tracing-subscriber` (stdout + file, JSON optional) | 0.5d |
| D8 | CLI args | `--config`, `--test`, `--once`, `--log-level`, `--log-file` | 0.5d |
| D9 | Integration tests | Daemon + mock backend (unit), daemon + X11 backend (Xvfb e2e) | 1d |

### Crate: keymouse-settings

| ID | Task | Description | Est. |
|----|------|-------------|------|
| S1 | Update imports | Use `keymouse-core::Config`, `keymouse-common::paths` | 0.5d |
| S2 | Extract key capture | Move inline `CaptureConn` → `key_capture.rs` | 0.5d |
| S3 | Daemon control | `start/stop/restart/status` using system APIs | 0.5d |
| S4 | Autostart module | `.desktop` file management | 0.5d |
| S5 | Theme module | Dark theme constants, visuals() | 0.5d |
| S6 | Config binding | Two-way binding UI ↔ Config struct | 0.5d |
| S7 | Smoke test | Ensure `--smoke-test` works with new structure | 0.5d |

### Workspace / DevOps

| ID | Task | Description | Est. |
|----|------|-------------|------|
| W1 | Workspace Cargo.toml | Define 4 crates, shared dependencies, profiles | 0.5d |
| W2 | CI pipeline | GitHub Actions: build, test (unit + Xvfb e2e), clippy, fmt | 1d |
| W3 | xtask (optional) | `cargo xtask release`, `cargo xtask deb` | 1d |
| W4 | Benchmarks | Binary size, RSS, startup latency vs v2.0 | 0.5d |
| W5 | Docs | Update README, ARCHITECTURE.md, CHANGELOG | 0.5d |

---

## Acceptance Criteria (v2.1)

| Criterion | Target |
|-----------|--------|
| **Binary size** | ≤ 1.2 MB (was 861 KB) — async runtime adds ~300 KB |
| **RAM (idle)** | ≤ 4 MB (was 2.6 MB) — Tokio runtime overhead |
| **Startup latency** | < 100 ms (was ~instant) — acceptable |
| **Unit test coverage** | ≥ 90% for `keymouse-core` |
| **E2E tests** | All pass under Xvfb (movement, click, drag, scroll, cancel) |
| **GUI smoke test** | Renders, captures keys, saves config, restarts daemon |
| **Hot-reload** | Edit `config.toml` → daemon applies within 500 ms, no restart |
| **Wayland ready** | Backend trait exists; stub compiles; no X11 types in core/daemon |
| **Error handling** | All `Result<T, E>` with `E: std::error::Error + Send + Sync + 'static` |
| **Logging** | `RUST_LOG=keymouse=debug` produces structured output |

---

## Risk Mitigation

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Tokio adds too much binary size | Medium | Medium | Use `tokio = { features = ["rt", "macros", "time", "signal"] }`, strip symbols |
| X11 event loop → async translation bugs | High | High | Keep v2.0 daemon as reference; extensive Xvfb e2e tests |
| Config hot-reload race conditions | Medium | High | Atomic swap of `Arc<Config>`, registry rebuild under lock |
| Action plugin overhead | Low | Low | Actions are trait objects; hot path is registry lookup (HashMap) |
| Breaking v1 config migration | Low | High | Preserve `migrate_ini()` exactly; add test |

---

## Future Extensions (Post v2.1)

- **Wayland backend** — Implement `Backend` for `smithay`/`wayland-client`
- **Scriptable actions** — Lua/WASM plugins for custom actions
- **DBus/IPC** — External control (e.g., `keymouse-cli toggle`)
- **Per-app profiles** — Different configs per focused window
- **Gesture support** — Multi-key chords for complex actions
- **Metrics endpoint** — `/metrics` for Prometheus scraping

---

## Appendix: Dependency Graph (v2.1)

```
keymouse-daemon
    │
    ├── keymouse-core (pure logic, actions, state, config)
    │       │
    │       └── keymouse-backend (Backend trait, X11 impl, mock)
    │
    └── keymouse-common (paths, logging, single-instance)

keymouse-settings
    │
    ├── keymouse-core (Config, keysym)
    └── keymouse-common (paths, daemon control, autostart)
```

No cycles. `keymouse-core` has zero platform deps. `keymouse-backend` only depends on `keymouse-core` for `CoreEvent`/`KeyCode` types.