#!/usr/bin/env python3
"""KeyMouse settings — a small modern settings window (tkinter, stdlib).

Change the hold-key, the click keys, and the three speeds, then press
«اعمال و ری‌استارت» — the GUI writes ~/.config/keymouse/config.ini
and restarts the daemon, so the new settings take effect at once.

Run:  keymouse-settings        (after install)
  or:  python3 keymouse_gui.py  (from the source folder)
"""
import os
import shutil
import signal
import subprocess
import sys
import time
import tkinter as tk
from tkinter import messagebox, ttk

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import keymouse  # noqa: E402  (the daemon module, for load/save_config)

CONFIG_PATH = os.path.expanduser("~/.config/keymouse/config.ini")
LAUNCHER = os.path.expanduser("~/.local/bin/keymouse")
AUTOSTART = os.path.expanduser("~/.config/autostart/keymouse.desktop")

BG = "#1e222b"
PANEL = "#282d38"
FG = "#e8eaf0"
MUTED = "#9aa3b2"
ACCENT = "#2dd4bf"
ACCENT_DARK = "#0f766e"
DANGER = "#f87171"
OK = "#4ade80"

MODIFIERS = [
    ("Ctrl راست — Right Ctrl", "Control_R"),
    ("Ctrl چپ — Left Ctrl", "Control_L"),
    ("Caps Lock", "Caps_Lock"),
    ("Alt راست — Right Alt", "Alt_R"),
    ("Super راست — Right Super", "Super_R"),
    ("Super چپ — Left Super", "Super_L"),
]
CLICK_KEYS = [
    ("Enter", "Return"),
    ("Space — فاصله", "space"),
    ("و (comma) ,", "comma"),
    ("نقطه (period) .", "period"),
    ("اسلش (slash) /", "slash"),
    ("نقطه‌ویرگول (semicolon) ;", "semicolon"),
    ("Enter نام‌پد", "KP_Enter"),
    ("Backspace", "BackSpace"),
]


def daemon_running():
    try:
        out = subprocess.run(["pgrep", "-f", r"keymouse\.py$"],
                             capture_output=True, text=True, timeout=5)
        return bool(out.stdout.strip())
    except Exception:
        return False


def daemon_stop():
    try:
        out = subprocess.run(["pgrep", "-f", r"keymouse\.py$"],
                             capture_output=True, text=True, timeout=5)
        for pid in out.stdout.split():
            try:
                os.kill(int(pid), signal.SIGTERM)
            except (ValueError, ProcessLookupError, PermissionError):
                pass
    except Exception:
        pass
    for _ in range(20):
        if not daemon_running():
            return
        time.sleep(0.1)


def daemon_start():
    here = os.path.dirname(os.path.abspath(__file__))
    candidates = [LAUNCHER, "/usr/local/bin/keymouse",
                  os.path.join(here, "keymouse.py")]
    cmd = None
    for cand in candidates:
        if os.path.exists(cand):
            cmd = [cand] if not cand.endswith(".py") else [sys.executable, cand]
            break
    if cmd is None:
        cmd = [LAUNCHER]
    log = os.path.expanduser("~/.local/share/keymouse/keymouse.log")
    os.makedirs(os.path.dirname(log), exist_ok=True)
    with open(log, "ab") as fh:
        subprocess.Popen(cmd, stdout=fh, stderr=fh,
                         start_new_session=True)


def autostart_enabled():
    return os.path.exists(AUTOSTART)


def set_autostart(enabled):
    os.makedirs(os.path.dirname(AUTOSTART), exist_ok=True)
    if enabled:
        exe = LAUNCHER if os.path.exists(LAUNCHER) else "/usr/local/bin/keymouse"
        with open(AUTOSTART, "w") as fh:
            fh.write("[Desktop Entry]\nType=Application\nName=KeyMouse\n"
                     "Comment=Hold a key and move the mouse with arrows\n"
                     f"Exec={exe}\nTerminal=false\n"
                     "X-XFCE-Autostart-enabled=true\n")
    elif os.path.exists(AUTOSTART):
        os.remove(AUTOSTART)


class Settings(tk.Tk):
    def __init__(self):
        super().__init__()
        self.title("KeyMouse — تنظیمات")
        self.geometry("540x720")
        self.configure(bg=BG)
        self.cfg = keymouse.load_config(CONFIG_PATH)
        self._build_style()
        self._build()
        self._refresh_status()
        self.after(2000, self._tick)

    # -- look -----------------------------------------------------------
    def _build_style(self):
        style = ttk.Style(self)
        try:
            style.theme_use("clam")
        except tk.TclError:
            pass
        style.configure("TFrame", background=BG)
        style.configure("Card.TFrame", background=PANEL)
        style.configure("TLabel", background=BG, foreground=FG,
                        font=("Sans", 11))
        style.configure("Card.TLabel", background=PANEL, foreground=FG,
                        font=("Sans", 11))
        style.configure("Muted.TLabel", background=BG, foreground=MUTED,
                        font=("Sans", 10))
        style.configure("Card.Muted.TLabel", background=PANEL,
                        foreground=MUTED, font=("Sans", 10))
        style.configure("Title.TLabel", background=BG, foreground=FG,
                        font=("Sans", 16, "bold"))
        style.configure("Section.TLabel", background=PANEL, foreground=ACCENT,
                        font=("Sans", 12, "bold"))
        style.configure("TButton", font=("Sans", 11), padding=8)
        style.configure("Accent.TButton", background=ACCENT,
                        foreground="#06251f", font=("Sans", 11, "bold"))
        style.configure("TCombobox", font=("Sans", 11), padding=6)
        style.configure("TCheckbutton", background=BG, foreground=FG,
                        font=("Sans", 11))

    def _card(self, parent, title):
        frame = ttk.Frame(parent, style="Card.TFrame", padding=14)
        frame.pack(fill="x", padx=14, pady=8)
        ttk.Label(frame, text=title, style="Section.TLabel").pack(anchor="w")
        return frame

    def _slider(self, parent, label, from_, to, key, fmt="{:.0f}", step=1):
        row = ttk.Frame(parent, style="Card.TFrame")
        row.pack(fill="x", pady=4)
        ttk.Label(row, text=label, style="Card.TLabel").pack(side="left")
        val = ttk.Label(row, text="", style="Card.TLabel")
        val.pack(side="right")
        var = tk.DoubleVar(value=float(self.cfg[key]))
        # resolution=step is essential: tk.Scale defaults to resolution
        # 1, which rounded the 0.25 slow multiplier down to 0.0.
        scale = tk.Scale(parent, variable=var, from_=from_, to=to,
                         orient="horizontal", showvalue=False,
                         resolution=step,
                         bg=PANEL, fg=FG, troughcolor="#3a4152",
                         highlightthickness=0, activebackground=ACCENT,
                         length=440)
        scale.pack(fill="x", pady=(0, 2))

        def _update(*_):
            v = var.get()
            if step == 1:
                v = round(v)
            val.config(text=fmt.format(v))
            self.cfg[key] = v if step != 1 else int(v)
            self._update_speed_preview()

        var.trace_add("write", _update)
        _update()
        return var

    # -- layout ---------------------------------------------------------
    def _build(self):
        root = ttk.Frame(self, padding=6)
        root.pack(fill="both", expand=True)
        ttk.Label(root, text="KeyMouse", style="Title.TLabel").pack(anchor="w", padx=14)
        ttk.Label(root, text="موس با کیبورد — نگه دار و با جهت‌ها حرکت بده",
                  style="Muted.TLabel").pack(anchor="w", padx=14, pady=(0, 4))

        # status card
        card = self._card(root, "وضعیت")
        row = ttk.Frame(card, style="Card.TFrame")
        row.pack(fill="x")
        self.status_dot = ttk.Label(row, text="●", style="Card.TLabel",
                                    foreground=MUTED)
        self.status_dot.pack(side="left")
        self.status_text = ttk.Label(row, text="…", style="Card.TLabel")
        self.status_text.pack(side="left", padx=6)
        ttk.Button(row, text="توقف", command=self._on_stop).pack(side="right", padx=4)
        ttk.Button(row, text="شروع", command=self._on_start).pack(side="right")
        self.autostart_var = tk.BooleanVar(value=autostart_enabled())
        ttk.Checkbutton(card, text="با ورود به سیستم خودکار شروع شود (Autostart)",
                        variable=self.autostart_var,
                        command=self._on_autostart,
                        style="TCheckbutton").pack(anchor="w", pady=(8, 0))

        # hold key + clicks
        card = self._card(root, "دکمه‌ها — Keybinding")
        ttk.Label(card, text="دکمه‌ای که نگه می‌داری:", style="Card.TLabel").pack(anchor="w")
        self.mod_var = tk.StringVar(value=self._label_for(MODIFIERS, self.cfg["modifier"]))
        ttk.Combobox(card, textvariable=self.mod_var,
                     values=[label for label, _ in MODIFIERS],
                     state="readonly").pack(fill="x", pady=4)
        for label, key in (("کلیک چپ:", "left_click_key"),
                           ("کلیک راست:", "right_click_key"),
                           ("کلیک وسط:", "middle_click_key")):
            ttk.Label(card, text=label, style="Card.TLabel").pack(anchor="w", pady=(6, 0))
            var = tk.StringVar(value=self._label_for(CLICK_KEYS, self.cfg[key]))
            setattr(self, f"{key}_var", var)
            ttk.Combobox(card, textvariable=var,
                         values=[l for l, _ in CLICK_KEYS],
                         state="readonly").pack(fill="x", pady=4)

        # speeds
        card = self._card(root, "سرعت پوینتر")
        ttk.Label(card, text="معمولی — فقط دکمه‌ی نگه‌دارنده (Ctrl):",
                  style="Card.TLabel").pack(anchor="w")
        self._slider(card, "سرعت شروع (px در هر تیک)", 1, 20, "base_speed")
        self._slider(card, "حداکثر سرعت (px در هر تیک)", 10, 100, "max_speed")
        self._slider(card, "شتاب (px در ثانیه)", 0, 200, "accel_per_sec", step=5)
        ttk.Label(card, text="سریع — دکمه + Shift:",
                  style="Card.TLabel").pack(anchor="w", pady=(10, 0))
        self._slider(card, "ضریب سرعت سریع", 1, 5, "fast_multiplier",
                     fmt="×{:.1f}", step=0.1)
        ttk.Label(card, text="دقیق — دکمه + Alt:",
                  style="Card.TLabel").pack(anchor="w", pady=(10, 0))
        self._slider(card, "ضریب سرعت دقیق", 0.05, 1, "slow_multiplier",
                     fmt="×{:.2f}", step=0.05)
        self.preview = ttk.Label(card, text="", style="Card.Muted.TLabel")
        self.preview.pack(anchor="w", pady=(8, 0))
        self._update_speed_preview()

        # actions
        row = ttk.Frame(root)
        row.pack(fill="x", padx=14, pady=12)
        ttk.Button(row, text="اعمال و ری‌استارت", style="Accent.TButton",
                   command=self._on_apply).pack(side="right", fill="x", expand=True)
        ttk.Button(row, text="بازنشانی", command=self._on_reset).pack(side="left", padx=6)

    def _label_for(self, options, keysym):
        for label, value in options:
            if value == keysym:
                return label
        return options[0][0]

    def _keysym_for(self, options, label):
        for text, value in options:
            if text == label:
                return value
        return options[0][1]

    def _update_speed_preview(self):
        # Sliders are built before the preview label exists; skip until then.
        if not hasattr(self, "preview"):
            return
        tick = self.cfg["tick_ms"] / 1000.0
        normal = self.cfg["max_speed"] / tick
        fast = normal * self.cfg["fast_multiplier"]
        slow = normal * self.cfg["slow_multiplier"]
        self.preview.config(
            text=f"حداکثر سرعت: معمولی {normal:.0f} پیکسل/ثانیه  •  "
                 f"سریع {fast:.0f}  •  دقیق {slow:.0f}")

    def _collect(self):
        self.cfg["modifier"] = self._keysym_for(MODIFIERS, self.mod_var.get())
        for key in ("left_click_key", "right_click_key", "middle_click_key"):
            var = getattr(self, f"{key}_var")
            self.cfg[key] = self._keysym_for(CLICK_KEYS, var.get())
        return dict(self.cfg)

    def _on_apply(self):
        values = self._collect()
        try:
            keymouse.save_config(CONFIG_PATH, values)
        except OSError as exc:
            messagebox.showerror("خطا", f"ذخیره‌ی تنظیمات نشد:\n{exc}")
            return
        daemon_stop()
        daemon_start()
        time.sleep(0.6)
        self._refresh_status()
        if daemon_running():
            messagebox.showinfo("KeyMouse", "تنظیمات اعمال شد و دیمون دوباره شروع شد.")
        else:
            messagebox.showwarning(
                "KeyMouse",
                "تنظیمات ذخیره شد ولی دیمون شروع نشد.\n"
                "لاگ را ببین: ~/.local/share/keymouse/keymouse.log")

    def _on_reset(self):
        self.cfg = keymouse.load_config(None)
        for widget in self.winfo_children():
            widget.destroy()
        self._build()
        self._refresh_status()

    def _on_start(self):
        daemon_start()
        time.sleep(0.5)
        self._refresh_status()

    def _on_stop(self):
        daemon_stop()
        self._refresh_status()

    def _on_autostart(self):
        set_autostart(self.autostart_var.get())

    def _refresh_status(self):
        running = daemon_running()
        self.status_dot.config(foreground=OK if running else DANGER)
        self.status_text.config(
            text="در حال اجراست — آماده‌ی حرکت با کیبورد" if running
            else "متوقف است — دکمه‌ی شروع را بزن")

    def _tick(self):
        self._refresh_status()
        self.after(2000, self._tick)


def main():
    app = Settings()
    app.mainloop()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
