# KeyMouse v2.1 — Task Checklist

## Phase 1: Foundation (Week 1-2)

### Crate: keymouse-core
- [ ] **C1** Create crate structure (`cargo new --lib keymouse-core`)
- [ ] **C2** Extract `config.rs` from v2.0 `src/config.rs`
  - [ ] Config struct + TOML serde
  - [ ] `load()`, `save()`, `migrate_ini()`, `sanitize()`
  - [ ] Default config constants
  - [ ] Unit tests: round-trip, migration, validation
- [ ] **C3** Extract `keysym.rs` from v2.0 `src/keysym.rs`
  - [ ] `KEYSYM_TABLE`, `keysym_from_name()`, `name_from_keysym()`
  - [ ] `display_name()`, `mirror_keysym()`
  - [ ] Unit tests: lookup, display names
- [ ] **C4** Extract `motion.rs` from v2.0 `src/logic.rs`
  - [ ] `Direction`, `HeldDirs`, `compute_move_step()`
  - [ ] `ease()`, acceleration curve
  - [ ] Unit tests: all `logic.rs` tests ported
- [ ] **C5** Create `action.rs` — Action trait + Registry
  - [ ] `Action` trait with `on_press`, `on_release`, `on_tick`
  - [ ] `ActionRegistry` with HashMap<KeyCode, Box<dyn Action>>
  - [ ] `ModifierKind` (Fast, Slow)
- [ ] **C6** Built-in actions (in `actions/` module)
  - [ ] `MoveAction` — arrow keys → movement with accel
  - [ ] `ClickAction` — left/right/middle with drag support
  - [ ] `ScrollAction` — hold-to-scroll (PageUp/PageDown)
  - [ ] `ModifierAction` — Shift (fast), Alt (slow)
  - [ ] `CancelAction` — Escape releases all
- [ ] **C7** Create `state.rs` — Pure DaemonState
  - [ ] `DaemonState` with all fields (active, dirs, modifiers, scroll, drag)
  - [ ] State transitions: `activate()`, `deactivate()`, `set_direction()`, etc.
  - [ ] No I/O, pure functions
- [ ] **C8** Core error types (`error.rs`)
  - [ ] `ConfigError`, `ActionError`, `StateError` with `thiserror`
  - [ ] `From` impls for std errors
- [ ] **C9** Unit tests for core
  - [ ] Config round-trip + migration
  - [ ] Keysym lookup
  - [ ] Motion computation (all cases)
  - [ ] Action registry: register, trigger, tick
  - [ ] State transitions

### Workspace Setup
- [ ] **W1** Workspace `Cargo.toml` with 4 crates
  - [ ] Shared dependencies (tokio, tracing, thiserror, async-trait, serde, notify, clap, x11rb, egui, eframe)
  - [ ] Profiles: release-lto, dev, test
  - [ ] Workspace metadata

## Phase 2: Backend Abstraction (Week 2-3)

### Crate: keymouse-backend
- [ ] **B1** Create crate + `Backend` trait
  - [ ] `CoreEvent`, `KeyCode`, `MouseButton`, `ScrollDirection`
  - [ ] `BackendError` with `thiserror`
- [ ] **B2** X11 connection wrapper
  - [ ] Async `connect()` → `x11rb::RustConnection`
  - [ ] Screen info, root window
- [ ] **B3** X11 keymap
  - [ ] Port `Keymap::fetch()` from `x11util.rs`
  - [ ] `keycode_for_name()`, `keysym_for_keycode()`
- [ ] **B4** X11 grab
  - [ ] `grab_key()`, `ungrab_key()`, `grab_keyboard()`, `ungrab_keyboard()`
  - [ ] Handle `GrabStatus` errors
- [ ] **B5** X11 input
  - [ ] `warp_pointer(dx, dy)`
  - [ ] `fake_button(button, press)` via XTest
  - [ ] `fake_key(keycode, press)` via XTest
- [ ] **B6** X11 events → CoreEvent
  - [ ] Poll X events, filter KeyPress/KeyRelease
  - [ ] Map keycode → CoreEvent::KeyPress/KeyRelease
  - [ ] Detect modifier press/release
- [ ] **B7** Mock backend
  - [ ] In-memory event queue
  - [ ] Controllable warp/click/scroll for testing
- [ ] **B8** Backend tests
  - [ ] X11 backend under Xvfb (movement, click, scroll)
  - [ ] Mock backend unit tests

## Phase 3: Action System (Week 3-4)

- [ ] Integrate `ActionRegistry` into daemon flow
- [ ] Wire built-in actions to registry at startup from Config
- [ ] Remove hard-coded `Action` enum from v2.0 daemon
- [ ] Test: each action triggers correctly via mock backend

## Phase 4: Async Daemon (Week 4-5)

### Crate: keymouse-daemon
- [ ] **D1** Create binary crate with Tokio
- [ ] **D2** `Daemon` struct
  - [ ] Fields: `backend`, `registry`, `state`, `config_watcher`, `shutdown_tx`
- [ ] **D3** Async event loop
  - [ ] `tokio::select!` on:
    - [ ] `backend.next_event()`
    - [ ] `tick_interval.tick()` (16ms default)
    - [ ] `config_watcher.next()`
    - [ ] `shutdown_rx.recv()`
- [ ] **D4** Config hot-reload
  - [ ] `notify` watcher on config file
  - [ ] On change: `Config::load()`, rebuild `ActionRegistry`, update speeds
- [ ] **D5** Signal handling
  - [ ] SIGINT/SIGTERM → `shutdown_tx.send(())`
  - [ ] On shutdown: release grabs, flush, exit cleanly
- [ ] **D6** Single instance lock
  - [ ] Port from v2.0 `data_dir()` + file lock
  - [ ] Cross-platform (fs2 or custom)
- [ ] **D7** Logging setup
  - [ ] `tracing_subscriber::fmt` with env filter
  - [ ] File + stdout, JSON optional
  - [ ] `RUST_LOG` support
- [ ] **D8** CLI args (clap)
  - [ ] `--config PATH`, `--test`, `--once`, `--log-level`, `--log-file`
  - [ ] `--test` runs one loop iteration and exits (for CI)
- [ ] **D9** Integration tests
  - [ ] Daemon + mock backend (unit)
  - [ ] Daemon + X11 backend (Xvfb e2e)

## Phase 5: GUI Updates (Week 5-6)

### Crate: keymouse-settings
- [ ] **S1** Update imports to new crates
- [ ] **S2** Extract `key_capture.rs`
  - [ ] X11 connection for key capture
  - [ ] `CaptureConn` with grab/ungrab
- [ ] **S3** `daemon_control.rs`
  - [ ] `start()`, `stop()`, `restart()`, `status()`
  - [ ] Uses system process management
- [ ] **S4** `autostart.rs`
  - [ ] `.desktop` file create/remove
  - [ ] XDG autostart dir
- [ ] **S5** `theme.rs`
  - [ ] Dark theme colors, spacing, visuals()
- [ ] **S6** Config binding
  - [ ] UI ↔ `Config` two-way sync
  - [ ] Save on change + daemon restart
- [ ] **S7** Smoke test verification
  - [ ] `--smoke-test` renders frames, saves config, exits

## Phase 6: Integration & Polish (Week 6-7)

- [ ] **Full e2e test suite** passes under Xvfb
  - [ ] Movement (up/down/left/right)
  - [ ] Acceleration curve
  - [ ] Fast/slow modifiers
  - [ ] Left/right/middle click + drag
  - [ ] Scroll up/down
  - [ ] Cancel (Escape)
  - [ ] Config hot-reload
- [ ] **Benchmark** vs v2.0
  - [ ] Binary size (release, stripped)
  - [ ] RSS at idle
  - [ ] Startup time
  - [ ] CPU during movement
- [ ] **Installer** updates if needed
  - [ ] Binary paths
  - [ ] Config location
- [ ] **Documentation**
  - [ ] README.md updated
  - [ ] ARCHITECTURE.md (this file)
  - [ ] CHANGELOG.md
- [ ] **CI Pipeline** (GitHub Actions)
  - [ ] Build (release + debug)
  - [ ] Unit tests (all crates)
  - [ ] Xvfb e2e tests
  - [ ] Clippy, rustfmt
  - [ ] Binary size check

---

## Dependency Order

```
W1 (workspace)
    │
    ├── C1 (core crate) ──→ C2, C3, C4 (extract) ──→ C5, C6, C7, C8, C9
    │
    ├── B1 (backend crate) ──→ B2, B3, B4, B5, B6 ──→ B7, B8
    │
    ├── D1 (daemon crate) ──→ D2, D3, D4, D5, D6, D7, D8 ──→ D9 (needs B8, C9)
    │
    └── S1 (settings crate) ──→ S2, S3, S4, S5, S6, S7 (needs C2, C3)
```

---

## Quick Start for Developers

```bash
# 1. Create workspace structure
cargo new --lib keymouse-core
cargo new --lib keymouse-backend
cargo new --bin keymouse-daemon
cargo new --bin keymouse-settings

# 2. Edit workspace Cargo.toml to include all 4 crates

# 3. Run tests frequently
cargo test -p keymouse-core
cargo test -p keymouse-backend -- --test-threads=1  # Xvfb tests need serial
cargo test -p keymouse-daemon -- --test-threads=1
cargo test -p keymouse-settings -- --test-threads=1
cargo test --workspace

# 4. Run daemon manually under Xvfb for debugging
Xvfb :99 -screen 0 1280x800x24 &
DISPLAY=:99 cargo run -p keymouse-daemon -- --test
```

---

## Definition of Done (per task)

- [ ] Code compiles (`cargo build --release`)
- [ ] All tests pass (`cargo test -p <crate>`)
- [ ] Clippy clean (`cargo clippy -p <crate> -- -D warnings`)
- [ ] Formatted (`cargo fmt -p <crate> -- --check`)
- [ ] Documentation for public APIs (`cargo doc -p <crate>`)