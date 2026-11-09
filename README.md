# KeyMouse v2.5 — Keyboard Mouse Controller for Linux / X11

Hold a single modifier key, steer the cursor with arrow keys. Lightweight, responsive, zero bloated background services.

> 📖 Persian Documentation: [README.fa.md](README.fa.md)

---

## ✨ What's New in v2.5

- **Universal Key Binding:** Bind **ANY** key on any keyboard (F1-F12, Insert, Delete, Home, End, NumLock, Media keys, or custom XKB keys) to the main modifier or any action.
- **UI/UX Pro Max Design:** Updated egui interface with dynamic version display (`v2.5.0`), live status badges, and refined dark theme.
- **Fast Hex Fallback:** Full support for custom keysyms and hex fallback for non-standard keyboard layouts.

---

## ⚡ Installation

Get the `.deb` from the [Releases](https://github.com/ali98afshari-star/keymouse/releases) page and install it:

```bash
sudo dpkg -i keymouse_2.0.0_amd64.deb
sudo apt install -f   # if it asks for dependencies
```

Then log out / log in, or just run `keymouse` in a terminal.

Or install from the tarball: extract it and run `./install.sh` (no root;
it just copies two binaries into `~/.local/bin`).

Or build from source:

```bash
git clone https://github.com/ali98afshari-star/keymouse.git && cd keymouse
cargo build --release && ./install.sh
```

**Upgrading from v1 (Python):** stop the old Python daemon first
(`pkill -f keymouse.py`) — `install.sh` does this for you. Your old
config (`~/.config/keymouse/config.ini`) is migrated automatically to
the new format (`config.toml`).

---

## 🎮 Controls & Shortcuts

| Hold + Key | Action |
|---|---|
| **Right Ctrl** (hold) + `↑ ↓ ← →` | Move cursor (supports smooth diagonals) |
| + `Shift` | Fast Mode (3x multiplier) |
| + `Alt` | Precision Mode (0.25x micro steps) |
| `Enter` (while holding Right Ctrl) | Left Click — hold it = drag |
| `.` (period) | Right Click |
| `/` (slash) | Middle Click |
| `PageUp` / `PageDown` | Scroll up / down (repeats while held) |
| `Esc` | Cancel / release everything |

Speed starts slow and ramps up the longer you hold a direction, so
precise aiming and fast screen-crossing both feel right.

> **Note:** As long as Right Ctrl is held, arrow keys and clicks are intercepted and do not interfere with background applications. Left Ctrl remains completely unaffected for normal typing.

---

## 🛠 Settings — modern dark window

Run from terminal or your Application Menu:

```bash
keymouse-settings
```

Features:
- **Game-style key mapping:** click a binding row, press the new key,
  done (Esc cancels). Works for every binding: the hold key, the four
  directions, fast, slow, the three clicks, the two scroll keys, cancel.
- High-contrast modern dark UI, clear sections
  (Status / Keybindings / Pointer speed / Behaviour)
- Configurable modifier key (`Right Ctrl`, `Left Ctrl`, `Caps Lock`,
  `Alt`, `Super`, …)
- Sliders for all speeds, with a live px/sec preview
  (normal / fast / precise)
- Daemon start/stop, autostart toggle, Apply & Restart, Reset to defaults

"Apply & Restart" writes the settings to
`~/.config/keymouse/config.toml` and restarts the daemon.

---

## ✨ Version 2 — complete rewrite in Rust

- **Much lighter:** the daemon is a single 861 KB binary — no Python,
  no venv, no `python-xlib`. It uses about **2.6 MB of RAM**
  (the Python version used ~16.6 MB — roughly 6× lighter).
  Startup is instant.
- **Everything is configurable** in the settings window and in the
  config file (`config.toml`): every key, every speed, acceleration,
  tick, scroll repeat.
- **New settings window** built with egui: dark theme, clear sections,
  live speed preview.
- Behaviour is exactly the same as v1.1 (same keys, same acceleration,
  same click anti-repeat, same drag safety, same scroll, same Esc,
  same single-instance lock).

The Python version (v1.2, with its own modern dark GUI and a separate
dedicated drag key) is still kept intact under `python-v1/`.

---

## Why build this, when ready-made tools exist?

- **X11's built-in MouseKeys** (`setxkbmap -option keypad:pointerkeys`,
  then Shift+NumLock) only works with the **numpad**, not the arrow keys —
  useless on a laptop without a numpad.
- **warpd** is modal (Alt+Meta+C to enter the mode, then hjkl) and
  hasn't been updated in three years; it has hang reports on XFCE.
- **keynav** splits the screen in halves to reach a point — no
  continuous movement.
- None of them were exactly "hold one key + arrows", so this was built.

---

## 🧪 Tests

```bash
keymouse --test        # pure-logic test (speed/direction/config), no X needed
cargo test --release   # full suite: 12 unit tests + an Xvfb e2e test
                       # (real pointer movement, click, scroll) + a GUI
                       # smoke test (window renders, config saves)
```

---

## 🗑 Uninstallation

```bash
./uninstall.sh        # from the tarball / source
# or, for the .deb:
sudo dpkg -r keymouse
```

---

## Limitations

- **X11 only.** Linux Mint XFCE uses X11 by default, so that's fine;
  it does not work on Wayland.
- If another program has grabbed the keyboard at the same time,
  keymouse cannot grab the hold key, and it says so in the log.
