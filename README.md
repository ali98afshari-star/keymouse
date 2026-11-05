# KeyMouse — Keyboard Mouse Controller for Linux / X11

Hold a single modifier key, steer the cursor with arrow keys. Lightweight, responsive, zero bloated background services.

> 📖 Persian Documentation: [README.fa.md](README.fa.md)

---

## ⚡ 1-Line Installation

Clone the repository and run the non-root installer:

```bash
git clone https://github.com/ali98afshari-star/keymouse.git && cd keymouse && ./install.sh
```

Or install the `.deb` package directly:

```bash
sudo dpkg -i keymouse_1.2_all.deb
sudo apt install -f
```

---

## 🎮 Controls & Shortcuts

| Hold + Key | Action |
|---|---|
| **Right Ctrl** (hold) + `↑ ↓ ← →` | Move cursor (supports smooth diagonals) |
| + `Shift` | Fast Mode (3x multiplier) |
| + `Alt` | Precision Mode (0.25x micro steps) |
| `Enter` | Left Click (tap) |
| `d` (or dedicated drag key) | **Hold to Drag** (avoids accidental file opening) |
| `.` (period) | Right Click |
| `/` (slash) | Middle Click |
| `PageUp` / `PageDown` | Continuous Scroll (smooth repeating) |
| `Esc` | Emergency release / Cancel grab |

> **Note:** As long as Right Ctrl is held, arrow keys and clicks are intercepted and do not interfere with background applications. Left Ctrl remains completely unaffected for normal typing.

---

## 🛠 Modern Settings Dashboard

Run from terminal or your Application Menu:

```bash
keymouse-settings
```

Features:
- High-contrast modern dark UI
- Configurable modifier key (`Right Ctrl`, `Caps Lock`, `AltGr`, etc.)
- Customizable click and drag bindings
- Real-time speed and acceleration sliders
- One-click autostart toggle on desktop login

---

## 🧪 Self Tests

Verify logic and acceleration curves without an active display server:

```bash
keymouse --test
```

---

## 🗑 Uninstallation

```bash
./uninstall.sh
```
