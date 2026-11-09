#!/bin/sh
# keymouse installer v2 — Linux Mint / XFCE (X11). No root needed.
# Rust binaries: no Python, no venv, no python-xlib.
# Installs the binaries to ~/.local/bin, autostart entry to
# ~/.config/autostart/keymouse.desktop, menu entry for the settings.
set -eu
SRC_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
BIN_DIR="$HOME/.local/bin"
CONFIG_DIR="$HOME/.config/keymouse"
CONFIG="$CONFIG_DIR/config.toml"
AUTOSTART_DIR="$HOME/.config/autostart"
LOG_DIR="$HOME/.local/share/keymouse"

mkdir -p "$BIN_DIR" "$CONFIG_DIR" "$AUTOSTART_DIR" "$LOG_DIR" "$HOME/.local/share/applications"

# --- binaries ----------------------------------------------------------
# From a tarball: keymouse + keymouse-settings sit next to this script.
# From the repo: they are in target/release/.
for _b in keymouse keymouse-settings; do
    if [ -x "$SRC_DIR/$_b" ]; then
        cp "$SRC_DIR/$_b" "$BIN_DIR/$_b"
    elif [ -x "$SRC_DIR/target/release/$_b" ]; then
        cp "$SRC_DIR/target/release/$_b" "$BIN_DIR/$_b"
    else
        echo "ERROR: $_b not found next to install.sh or in target/release/ — run 'cargo build --release' first."
        exit 1
    fi
    chmod +x "$BIN_DIR/$_b"
done

# --- menu entry for the settings GUI ------------------------------------
cat > "$HOME/.local/share/applications/keymouse-settings.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=KeyMouse Settings
Comment=Hold a key and move the mouse with the arrow keys — settings
Exec=$BIN_DIR/keymouse-settings
Terminal=false
Categories=Utility;Settings;
EOF

# --- default config (never overwrite an existing one) -----------------
# If a v1 config.ini exists, the daemon migrates it to config.toml
# automatically on first start; we do not touch it here.
if [ ! -f "$CONFIG" ]; then
    cat > "$CONFIG" <<'EOF'
# keymouse config (v2, TOML) — edit and restart keymouse,
# or change it in the settings window (keymouse-settings).
# Keys are X11 keysym names, e.g. Control_R, Up, Return.

modifier = "Control_R"
up = "Up"
down = "Down"
left = "Left"
right = "Right"
fast_keys = ["Shift_L", "Shift_R"]
slow_keys = ["Alt_L", "Alt_R"]
left_click = "Return"
right_click = "period"
middle_click = "slash"
scroll_up = "Prior"
scroll_down = "Next"
cancel = "Escape"
base_speed = 6.0
max_speed = 42.0
accel_per_sec = 90.0
tick_ms = 16
fast_multiplier = 3.0
slow_multiplier = 0.25
scroll_ms = 120
EOF
fi

# --- autostart on login (XFCE: Session and Startup reads this) --------
cat > "$AUTOSTART_DIR/keymouse.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=KeyMouse
Comment=Hold Right Ctrl and move the mouse with the arrow keys
Exec=$BIN_DIR/keymouse
Terminal=false
X-XFCE-Autostart-enabled=true
EOF

# --- stop any old daemon (v1 Python AND v2 Rust) and restart -----------
# Old Python daemon: its command line ends with keymouse.py.
for _pid in $(pgrep -f 'keymouse\.py$' 2>/dev/null || true); do
    kill "$_pid" 2>/dev/null || true
done
# Rust daemon: exact process name keymouse.
for _pid in $(pgrep -x keymouse 2>/dev/null || true); do
    kill "$_pid" 2>/dev/null || true
done
sleep 0.3
if [ -n "${DISPLAY:-}" ]; then
    nohup "$BIN_DIR/keymouse" > "$LOG_DIR/keymouse.log" 2>&1 &
    sleep 0.5
    if pgrep -x keymouse >/dev/null 2>&1; then
        echo "keymouse is running now. Hold Right Ctrl + arrow keys to move the mouse."
    else
        echo "keymouse did not start — check $LOG_DIR/keymouse.log (are you on X11, not Wayland?)"
    fi
else
    echo "Installed. No X display in this shell, so it will start at your next login (autostart is set)."
fi
echo "Daemon: $BIN_DIR/keymouse   |   settings: $BIN_DIR/keymouse-settings   |   config: $CONFIG   |   test: $BIN_DIR/keymouse --test"
