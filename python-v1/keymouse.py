#!/usr/bin/env python3
"""keymouse — hold one key, move the mouse with arrow keys (X11 / XFCE).

Ponytail note: the lazy option is X11 MouseKeys (numpad only, not arrows),
and warpd/keynav are modal, not hold-a-key + arrows. Neither matches the
requested behaviour exactly, so this is the smallest thing that does:
one file, one dependency (python-xlib), no root, no system packages.

How it works
------------
* On start it grabs ONLY the modifier key (default: right Ctrl) on the
  root window. Normal typing is untouched.
* When you press-and-hold the modifier it grabs the whole keyboard.
  While grabbed, arrow keys move the pointer and never reach the app
  underneath (so Ctrl+Arrow / Ctrl+Enter can't fire app shortcuts).
* When you release the modifier it ungrabs the keyboard, releases any
  held mouse button (safety, so a drag can't get stuck) and goes back
  to just watching for the modifier.

Default keys (change them in ~/.config/keymouse/config.ini, or with
the settings GUI: keymouse-settings):
    hold Right Ctrl + Up/Down/Left/Right   move (diagonals work)
    + Shift (either) while moving           fast  x3
    + Alt (either) while moving             slow, precise
    Enter (while held)                      left button down/up = click / drag
    .  (period, while held)                 right click
    /  (slash, while held)                  middle click
    PageUp / PageDown (while held)          scroll up / down (repeats)
    Esc (while held)                        cancel / release everything

Speed ramps up the longer you hold a direction: it starts slow for
precise aiming and accelerates to full speed.

Usage:  keymouse [--config PATH] [--test] [--once]
  --test   run the pure-logic self-checks (speed/direction/config)
           without touching X; exits 0 on success.
  --once   move the pointer 1 px and exit (smoke test, needs X).
"""
import configparser
import math
import os
import signal
import sys
import threading
import time

from Xlib import X, XK, display as xdisplay
from Xlib.ext import xtest

DEFAULT_CONFIG = """\
# keymouse config — edit and restart keymouse (or log out/in).
# modifier: keysym name of the key you HOLD, e.g. Control_R, Caps_Lock,
# Alt_R, Super_R.  Control_R (right Ctrl) is the default: it is rarely
# used by apps, and left Ctrl keeps working normally.
# left/right/middle_click_key: keysym used for that button while held.
# drag_key: separate key for drag (hold to drag, tap to click). Empty = use left_click_key.
[mouse]
modifier = Control_R
base_speed = 6
max_speed = 42
accel_per_sec = 90
tick_ms = 16
fast_multiplier = 3.0
slow_multiplier = 0.25
scroll_ms = 120
left_click_key = Return
right_click_key = period
middle_click_key = slash
drag_key = 
"""

# Fixed keys hijacked while the modifier is held: name -> keysym name.
FIXED_KEYS = {
    "up": "Up", "down": "Down", "left": "Left", "right": "Right",
    "shift": "Shift_L", "shift_r": "Shift_R",
    "alt": "Alt_L", "alt_r": "Alt_R",
    "scroll_up": "Prior", "scroll_down": "Next",
    "cancel": "Escape",
}


def _clamp(value, lo, hi, default):
    try:
        v = float(value)
    except (TypeError, ValueError):
        return default
    return max(lo, min(hi, v))


def load_config(path):
    """Read config, falling back to defaults for any bad/missing value
    instead of crashing on a hand-edited file."""
    cfg = configparser.ConfigParser()
    cfg.read_string(DEFAULT_CONFIG)
    if path and os.path.exists(path):
        try:
            cfg.read(path)
        except configparser.Error as exc:
            print(f"keymouse: WARNING: bad config {path} ({exc}); "
                  "using defaults", file=sys.stderr)
    m = cfg["mouse"]
    base = _clamp(m.get("base_speed", 6), 1, 50, 6)
    maximum = _clamp(m.get("max_speed", 42), base, 200, 42)
    return {
        "modifier": m.get("modifier", "Control_R").strip() or "Control_R",
        "base_speed": base,
        "max_speed": maximum,
        "accel_per_sec": _clamp(m.get("accel_per_sec", 90), 0, 500, 90),
        "tick_ms": int(_clamp(m.get("tick_ms", 16), 5, 100, 16)),
        "fast_multiplier": _clamp(m.get("fast_multiplier", 3.0), 1, 5, 3.0),
        "slow_multiplier": _clamp(m.get("slow_multiplier", 0.25), 0.05, 1, 0.25),
        "scroll_ms": int(_clamp(m.get("scroll_ms", 120), 30, 1000, 120)),
        "left_click_key": m.get("left_click_key", "Return").strip() or "Return",
        "right_click_key": m.get("right_click_key", "period").strip() or "period",
        "middle_click_key": m.get("middle_click_key", "slash").strip() or "slash",
        "drag_key": m.get("drag_key", "").strip() or "",
    }


def save_config(path, values):
    """Write a config dict in the same format load_config() reads."""
    cfg = configparser.ConfigParser()
    cfg["mouse"] = {k: str(v) for k, v in values.items()}
    os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
    with open(path, "w") as fh:
        cfg.write(fh)


def _hijacked_keys(cfg):
    """Full hijacked-key map for this config (fixed keys + click keys)."""
    keys = dict(FIXED_KEYS)
    keys["left_click"] = cfg["left_click_key"]
    keys["right_click"] = cfg["right_click_key"]
    keys["middle_click"] = cfg["middle_click_key"]
    if cfg["drag_key"]:
        keys["drag"] = cfg["drag_key"]
    return keys


def speed_at(held_seconds, cfg):
    """Current px-per-tick: ramps from base to max. Pure, unit-tested."""
    return min(cfg["max_speed"],
               cfg["base_speed"] + cfg["accel_per_sec"] * held_seconds)


def direction_vector(held):
    """held: set of 'up'/'down'/'left'/'right' -> unit (dx, dy), or (0, 0)."""
    dx = (1 if "right" in held else 0) - (1 if "left" in held else 0)
    dy = (1 if "down" in held else 0) - (1 if "up" in held else 0)
    if dx and dy:  # diagonal: keep the same overall speed
        dx /= math.sqrt(2)
        dy /= math.sqrt(2)
    return dx, dy


def move_step(held, held_seconds, fast, slow, cfg):
    """One tick of movement: (dx, dy) in whole pixels. Pure, unit-tested."""
    dx, dy = direction_vector(held)
    if dx == 0 and dy == 0:
        return 0, 0
    speed = speed_at(held_seconds, cfg)
    if slow:
        # Precise mode wins when both are held: fast x slow was a bug
        # (it silently cancelled the slowdown to 0.75x).
        speed *= cfg["slow_multiplier"]
    elif fast:
        speed *= cfg["fast_multiplier"]
    return int(round(dx * speed)), int(round(dy * speed))


class KeyMouse:
    def __init__(self, cfg, config_path):
        self.cfg = cfg
        self.config_path = config_path
        self.disp = xdisplay.Display()
        self.root = self.disp.screen().root
        self.screen = self.disp.screen()
        self.held_dirs = set()       # subset of up/down/left/right
        self.held_start = None       # time the current direction started
        self.fast_keys = set()       # shift keys currently held
        self.slow_keys = set()       # alt keys currently held
        self.scroll_dirs = set()     # scroll_up / scroll_down held
        self.pressed_clicks = set()  # click keys currently down (anti-repeat)
        self.active = False          # modifier held => keyboard grabbed
        self.left_down = False
        self.drag_down = False
        self.last_scroll = 0.0
        self.stop = threading.Event()
        self._init_xtest()
        self.keycodes = {}
        for name, sym in [("modifier", cfg["modifier"])] + list(_hijacked_keys(cfg).items()):
            kc = self.disp.keysym_to_keycode(XK.string_to_keysym(sym))
            if not kc:
                print(f"keymouse: WARNING: no keycode for keysym {sym!r} "
                      f"({name}), that key will not work", file=sys.stderr)
            self.keycodes[name] = kc
        if not self.keycodes.get("modifier"):
            raise SystemExit(
                f"keymouse: ERROR: modifier keysym "
                f"{cfg['modifier']!r} has no keycode on this keyboard; "
                "fix 'modifier' in the config and restart")
        self.code_to_name = {kc: n for n, kc in self.keycodes.items()
                             if kc}
        # No extra window is needed: the passive grab on the modifier
        # key (see run()) delivers events to the root window.

    def _init_xtest(self):
        # xtest.fake_input() needs no init() call; just check the
        # extension is present. XTest missing: movement still works.
        try:
            self.xtest_ok = bool(self.disp.has_extension("XTEST"))
        except Exception as exc:
            print(f"keymouse: WARNING: XTest unavailable ({exc}); "
                  "clicks/scroll disabled, movement still works",
                  file=sys.stderr)
            self.xtest_ok = False
        if not self.xtest_ok:
            print("keymouse: WARNING: XTest unavailable; "
                  "clicks/scroll disabled, movement still works",
                  file=sys.stderr)

    # -- mouse actions -------------------------------------------------
    def move_relative(self, dx, dy):
        if dx or dy:
            self.disp.warp_pointer(dx, dy)
            self.disp.sync()

    def click(self, button, press):
        if not self.xtest_ok:
            return
        xtest.fake_input(self.disp, X.ButtonPress if press else X.ButtonRelease,
                         detail=button)
        self.disp.sync()

    def scroll(self, button):
        self.click(button, True)
        self.click(button, False)

    # -- mover thread --------------------------------------------------
    def _mover(self):
        while self.active and not self.stop.is_set():
            if self.held_dirs:
                if self.held_start is None:
                    self.held_start = time.monotonic()
                held_s = time.monotonic() - self.held_start
                dx, dy = move_step(self.held_dirs, held_s,
                                   bool(self.fast_keys),
                                   bool(self.slow_keys), self.cfg)
                self.move_relative(dx, dy)
            else:
                self.held_start = None
            # Hold-to-scroll: previously scrolling only fired on the
            # initial key press and relied on X auto-repeat, which is
            # off on some systems. Now it repeats every scroll_ms.
            if self.scroll_dirs:
                now = time.monotonic()
                if now - self.last_scroll >= self.cfg["scroll_ms"] / 1000.0:
                    self.last_scroll = now
                    if "scroll_up" in self.scroll_dirs:
                        self.scroll(4)
                    if "scroll_down" in self.scroll_dirs:
                        self.scroll(5)
            self.stop.wait(self.cfg["tick_ms"] / 1000.0)

    # -- grabbed-key handling ------------------------------------------
    def _on_key(self, ev, is_press):
        name = self.code_to_name.get(ev.detail)
        if name is None:
            return
        if name == "modifier":
            if not is_press:
                self._deactivate()
            return
        if name in ("up", "down", "left", "right"):
            if is_press:
                if not self.held_dirs:
                    self.held_start = time.monotonic()
                self.held_dirs.add(name)
            else:
                self.held_dirs.discard(name)
                if not self.held_dirs:
                    self.held_start = None
        elif name in ("shift", "shift_r"):
            if is_press:
                self.fast_keys.add(name)
            else:
                self.fast_keys.discard(name)
        elif name in ("alt", "alt_r"):
            if is_press:
                self.slow_keys.add(name)
            else:
                self.slow_keys.discard(name)
        elif name == "left_click":
            # Left button: tap to click, hold to drag.
            # X auto-repeat sends extra KeyPress events while held;
            # ignore them, or the button gets pressed again and again mid-drag.
            if is_press and name not in self.pressed_clicks:
                self.pressed_clicks.add(name)
                self.left_down = True
                self.click(1, True)
            elif not is_press:
                self.pressed_clicks.discard(name)
                self.left_down = False
                self.click(1, False)
        elif name == "drag":
            # Dedicated drag key: hold to drag, tap to click (no drag).
            # This prevents accidental file opens when trying to drag.
            if is_press and name not in self.pressed_clicks:
                self.pressed_clicks.add(name)
                self.drag_down = True
                self.click(1, True)
            elif not is_press:
                self.pressed_clicks.discard(name)
                self.drag_down = False
                self.click(1, False)
        elif name in ("right_click", "middle_click"):
            # Same anti-repeat: one tap = one click, even if X repeats.
            if is_press and name not in self.pressed_clicks:
                self.pressed_clicks.add(name)
                button = 3 if name == "right_click" else 2
                self.click(button, True)
                self.click(button, False)
            elif not is_press:
                self.pressed_clicks.discard(name)
        elif name in ("scroll_up", "scroll_down"):
            if is_press:
                self.scroll_dirs.add(name)
                self.last_scroll = 0.0  # scroll once immediately
            else:
                self.scroll_dirs.discard(name)
        elif name == "cancel" and is_press:
            self._deactivate()

    def _activate(self):
        # No explicit grab_keyboard() here: the passive grab on the
        # modifier key (grab_key below, owner_events=False) already
        # grabs the keyboard while the modifier is held, and delivers
        # every following key event to the root window. Calling
        # grab_keyboard() again at this point fails with BadAccess
        # because the keyboard is already grabbed by that passive grab.
        self.active = True
        self.held_dirs = set()
        self.held_start = None
        self.fast_keys = set()
        self.slow_keys = set()
        self.scroll_dirs = set()
        self.pressed_clicks = set()
        self.last_scroll = 0.0
        threading.Thread(target=self._mover, daemon=True).start()
        print("keymouse: active — arrows move the pointer, "
              "release the modifier to stop", flush=True)

    def _deactivate(self):
        if not self.active:
            return
        self.active = False
        if self.left_down:  # safety: never leave a drag stuck down
            self.click(1, False)
            self.left_down = False
        if self.drag_down:
            self.click(1, False)
            self.drag_down = False
        self.held_dirs = set()
        self.fast_keys = set()
        self.slow_keys = set()
        self.scroll_dirs = set()
        self.pressed_clicks = set()
        # The passive grab ends by itself when the modifier is released;
        # no ungrab_keyboard() call (it would fail / do nothing useful).

    # -- main loop ------------------------------------------------------
    def run(self):
        mod_kc = self.keycodes["modifier"]
        self.root.change_attributes(event_mask=X.KeyPressMask | X.KeyReleaseMask)
        # owner_events=False: while the modifier is held (passive grab
        # active), ALL key events are delivered to the root window, so
        # the daemon sees the arrows and the apps underneath see nothing.
        self.root.grab_key(mod_kc, X.AnyModifier, False,
                           X.GrabModeAsync, X.GrabModeAsync)
        print(f"keymouse: ready — hold {self.cfg['modifier']} + arrows "
              f"to move the mouse (config: {self.config_path})", flush=True)
        while not self.stop.is_set():
            ev = self.disp.next_event()
            if ev.type == X.KeyPress:
                if ev.detail == mod_kc and not self.active:
                    self._activate()
                elif self.active:
                    self._on_key(ev, True)
            elif ev.type == X.KeyRelease:
                if self.active:
                    self._on_key(ev, False)
            # KeyRelease of the modifier while NOT grabbed can't happen:
            # the grab only starts on its press.


def self_test():
    """Pure-logic checks, no X needed. Fails loudly on any regression."""
    cfg = load_config(None)
    assert cfg["modifier"] == "Control_R", cfg
    assert cfg["left_click_key"] == "Return", cfg
    assert speed_at(0, cfg) == 6, speed_at(0, cfg)
    assert speed_at(10, cfg) == 42, speed_at(10, cfg)  # capped at max
    assert direction_vector({"up"}) == (0, -1)
    assert direction_vector({"left", "down"})[0] < 0
    d = direction_vector({"up", "right"})
    assert abs((d[0] ** 2 + d[1] ** 2) ** 0.5 - 1) < 1e-9, d  # diagonal = unit
    assert move_step(set(), 0, False, False, cfg) == (0, 0)
    assert move_step({"right"}, 0, False, False, cfg) == (6, 0)
    assert move_step({"right"}, 0, True, False, cfg) == (18, 0)  # fast x3
    assert move_step({"right"}, 0, False, True, cfg) == (2, 0)   # slow: 6*.25=1.5->2
    assert move_step({"right"}, 0, True, True, cfg) == (2, 0)    # slow wins
    assert move_step({"up"}, 0, False, False, cfg) == (0, -6)
    # Bad values in a hand-edited config fall back, never crash.
    import tempfile
    with tempfile.NamedTemporaryFile("w", suffix=".ini", delete=False) as fh:
        fh.write("[mouse]\nbase_speed = banana\nmax_speed = 5\n")
        bad_path = fh.name
    bad = load_config(bad_path)
    assert bad["base_speed"] == 6, bad
    assert bad["max_speed"] >= bad["base_speed"], bad
    assert _parse_args(["keymouse", "--config"], quiet=True)[0] is None  # missing value
    print("keymouse --test: all self-checks passed")
    return 0


def _parse_args(argv, quiet=False):
    config_path = os.path.expanduser("~/.config/keymouse/config.ini")
    args = argv[1:]
    if "--config" in args:
        i = args.index("--config")
        if i + 1 >= len(args):
            if not quiet:
                print("keymouse: ERROR: --config needs a path after it",
                      file=sys.stderr)
            return None, args
        config_path = os.path.expanduser(args[i + 1])
    return config_path, args


def _single_instance():
    """Refuse a second daemon: two of them grab the same modifier key
    and the second one silently never activates (BadAccess)."""
    import fcntl
    lock_path = os.path.expanduser("~/.local/share/keymouse/keymouse.lock")
    os.makedirs(os.path.dirname(lock_path), exist_ok=True)
    fh = open(lock_path, "w")
    try:
        fcntl.flock(fh, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        print("keymouse: already running — refusing a second copy "
              "(kill the old one first: pkill -f 'keymouse.py$')",
              file=sys.stderr)
        raise SystemExit(1)
    return fh  # keep referenced: the lock dies with this process


def main(argv):
    config_path, args = _parse_args(argv)
    if config_path is None:
        return 2
    if "--test" in args:
        return self_test()
    cfg = load_config(config_path)
    if "--once" in args:  # smoke test: needs X, moves 1 px, exits
        km = KeyMouse(cfg, config_path)
        km.move_relative(1, 0)
        print("keymouse --once: moved +1px ok")
        return 0
    _lock = _single_instance()
    km = KeyMouse(cfg, config_path)

    def _quit(signum, frame):
        km._deactivate()
        km.stop.set()

    signal.signal(signal.SIGINT, _quit)
    signal.signal(signal.SIGTERM, _quit)
    try:
        km.run()
    except KeyboardInterrupt:
        pass
    finally:
        km._deactivate()
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))