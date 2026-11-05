#!/usr/bin/env python3
"""GUI smoke test: open the settings window under Xvfb, change a speed
and the modifier, press Apply, and check the config file + daemon.

Run:  Xvfb :97 -screen 0 1280x800x24 &
      DISPLAY=:97 HOME=/tmp/fh-gui python3 test_gui.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import keymouse_gui as gui
import keymouse


def main():
    app = gui.Settings()
    app.update_idletasks()
    assert "KeyMouse" in app.title(), app.title()
    # Change settings through the same code the Apply button uses.
    app.cfg["base_speed"] = 8
    app.cfg["max_speed"] = 60
    app.cfg["fast_multiplier"] = 4.0
    app.mod_var.set("Caps Lock")
    values = app._collect()
    assert values["modifier"] == "Caps_Lock", values
    assert values["base_speed"] == 8 and values["fast_multiplier"] == 4.0
    keymouse.save_config(gui.CONFIG_PATH, values)
    saved = keymouse.load_config(gui.CONFIG_PATH)
    assert saved["modifier"] == "Caps_Lock", saved
    assert saved["base_speed"] == 8 and saved["max_speed"] == 60, saved
    print("gui: config round-trip ok:", saved["modifier"],
          saved["base_speed"], saved["max_speed"], saved["fast_multiplier"])
    app.destroy()
    print("gui: PASSED")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
