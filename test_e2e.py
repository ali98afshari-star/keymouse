#!/usr/bin/env python3
"""End-to-end check for keymouse under a fake X server (Xvfb).

Run:  Xvfb :99 -screen 0 1280x800x24 &
      DISPLAY=:99 python3 keymouse.py &   # in another shell
      DISPLAY=:99 python3 test_e2e.py     # this file
It fakes holding Right Ctrl + Up, then Right Ctrl + Right, and checks
the pointer actually moved in the expected directions. Exits non-zero
on failure. Needs python-xlib only.
"""
import os
import sys
import time

from Xlib import X, XK, display as xdisplay
from Xlib.ext import xtest


def pos(disp, root):
    p = root.query_pointer()
    return p.root_x, p.root_y


def key(disp, keysym_name, press):
    kc = disp.keysym_to_keycode(XK.string_to_keysym(keysym_name))
    assert kc, f"no keycode for {keysym_name}"
    xtest.fake_input(disp, X.KeyPress if press else X.KeyRelease, detail=kc)
    disp.sync()


def main():
    disp = xdisplay.Display()
    root = disp.screen().root
    time.sleep(0.5)  # let the daemon grab the modifier key
    x0, y0 = pos(disp, root)
    print(f"start: {x0},{y0}")

    key(disp, "Control_R", True)
    time.sleep(0.2)
    key(disp, "Up", True)
    time.sleep(0.6)
    key(disp, "Up", False)
    key(disp, "Control_R", False)
    time.sleep(0.3)
    x1, y1 = pos(disp, root)
    print(f"after Up: {x1},{y1}  (dy={y1 - y0})")
    assert y1 < y0 - 20, f"pointer did not move up enough: dy={y1 - y0}"
    assert abs(x1 - x0) <= 2, f"pointer drifted sideways: dx={x1 - x0}"

    key(disp, "Control_R", True)
    time.sleep(0.2)
    key(disp, "Right", True)
    time.sleep(0.6)
    key(disp, "Right", False)
    # click test: Enter while held must not crash the daemon
    key(disp, "Return", True)
    time.sleep(0.1)
    key(disp, "Return", False)
    key(disp, "Control_R", False)
    time.sleep(0.3)
    x2, y2 = pos(disp, root)
    print(f"after Right: {x2},{y2}  (dx={x2 - x1})")
    assert x2 > x1 + 20, f"pointer did not move right enough: dx={x2 - x1}"

    print("e2e: PASSED — hold Right Ctrl + arrows moves the pointer")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
