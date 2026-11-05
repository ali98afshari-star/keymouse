#!/bin/sh
# keymouse uninstaller — removes the daemon, launcher, autostart and venv.
# Your config file is left in place; delete it by hand if you want it gone.
set -eu
for _pid in $(pgrep -f 'keymouse\.py$' 2>/dev/null || true); do
    kill "$_pid" 2>/dev/null || true
done
rm -f "$HOME/.local/bin/keymouse" "$HOME/.local/bin/keymouse-settings"
rm -f "$HOME/.config/autostart/keymouse.desktop"
rm -f "$HOME/.local/share/applications/keymouse-settings.desktop"
rm -rf "$HOME/.local/share/keymouse"
echo "keymouse removed. (Config kept at ~/.config/keymouse/config.ini — delete it if you like.)"
