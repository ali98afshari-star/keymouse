#!/bin/sh
# keymouse uninstaller v2 — removes the daemon, settings, launchers,
# autostart entry and log files.
# Your config file is left in place; delete it by hand if you want it gone.
set -eu
# Stop the Rust daemon (exact process name).
for _pid in $(pgrep -x keymouse 2>/dev/null || true); do
    kill "$_pid" 2>/dev/null || true
done
# Also stop a leftover v1 Python daemon, if any.
for _pid in $(pgrep -f 'keymouse\.py$' 2>/dev/null || true); do
    kill "$_pid" 2>/dev/null || true
done
rm -f "$HOME/.local/bin/keymouse" "$HOME/.local/bin/keymouse-settings"
rm -f "$HOME/.config/autostart/keymouse.desktop"
rm -f "$HOME/.local/share/applications/keymouse-settings.desktop"
rm -rf "$HOME/.local/share/keymouse"
echo "keymouse removed. (Config kept at ~/.config/keymouse/config.toml — delete it if you like.)"
