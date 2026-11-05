#!/usr/bin/env python3
"""keymouse-settings — Modern Dark UI for KeyMouse settings.

Clean, high-contrast dark theme with consistent modern layout.
"""
import configparser
import os
import subprocess
import sys
import tkinter as tk
from tkinter import ttk

CONFIG_PATH = os.path.expanduser("~/.config/keymouse/config.ini")

# Modifier choices: (English label, keysym name)
MODIFIERS = [
    ("Right Control (recommended)", "Control_R"),
    ("Caps Lock", "Caps_Lock"),
    ("Left Control", "Control_L"),
    ("Right Alt / AltGr", "Alt_R"),
    ("Left Super / Win", "Super_L"),
    ("Right Super", "Super_R"),
]

# Click/action key choices: (English label, keysym name)
CLICK_KEYS = [
    ("Enter", "Return"),
    ("Space", "space"),
    (". (period)", "period"),
    ("/ (slash)", "slash"),
    ("f", "f"),
    ("d", "d"),
    ("v", "v"),
    ("c", "c"),
]

DRAG_KEYS = [
    ("(Same as Left Click)", ""),
    ("d", "d"),
    ("Space", "space"),
    ("v", "v"),
    ("c", "c"),
    ("Alt", "Alt_L"),
]

# Ultra-modern dark palette
THEME = {
    "bg": "#0f172a",          # Deep slate background
    "card": "#1e293b",        # Card background
    "card_border": "#334155", # Card border
    "fg": "#f8fafc",          # Primary text
    "fg_dim": "#94a3b8",      # Secondary text
    "accent": "#38bdf8",      # Sky blue accent
    "accent_hover": "#0284c7",
    "success": "#10b981",     # Emerald green
    "danger": "#ef4444",      # Rose red
    "input_bg": "#0f172a",    # Dark input field
    "font_family": "Segoe UI, Cantarell, Ubuntu, sans-serif",
}


def daemon_running():
    out = subprocess.run(["pgrep", "-f", "keymouse.py$"],
                         capture_output=True).stdout
    return bool(out.strip())


def autostart_enabled():
    path = os.path.expanduser("~/.config/autostart/keymouse.desktop")
    return os.path.exists(path)


def set_autostart(enable):
    path = os.path.expanduser("~/.config/autostart/keymouse.desktop")
    if enable:
        os.makedirs(os.path.dirname(path), exist_ok=True)
        exe = os.path.expanduser("~/.local/bin/keymouse")
        content = f"""[Desktop Entry]
Type=Application
Name=KeyMouse
Comment=Control mouse with keyboard
Exec={exe}
Hidden=false
NoDisplay=false
X-GNOME-Autostart-enabled=true
"""
        with open(path, "w") as fh:
            fh.write(content)
    else:
        try:
            os.remove(path)
        except OSError:
            pass


class ModernSettings(tk.Tk):
    def __init__(self):
        super().__init__()
        self.title("KeyMouse Settings")
        self.geometry("540x680")
        self.minsize(500, 620)
        self.configure(bg=THEME["bg"])
        
        self._load_config()
        self._setup_styles()
        self._build_ui()
        self._refresh_status()

    def _setup_styles(self):
        style = ttk.Style(self)
        style.theme_use("clam")
        
        # Configure TTK widgets
        style.configure("TFrame", background=THEME["bg"])
        style.configure("Card.TFrame", background=THEME["card"], relief="solid", borderwidth=1)
        
        style.configure("TLabel", background=THEME["card"], foreground=THEME["fg"],
                        font=(THEME["font_family"], 10))
        style.configure("Title.TLabel", background=THEME["bg"], foreground=THEME["fg"],
                        font=(THEME["font_family"], 16, "bold"))
        style.configure("Subtitle.TLabel", background=THEME["bg"], foreground=THEME["fg_dim"],
                        font=(THEME["font_family"], 10))
        style.configure("Header.TLabel", background=THEME["card"], foreground=THEME["accent"],
                        font=(THEME["font_family"], 11, "bold"))
        style.configure("Dim.TLabel", background=THEME["card"], foreground=THEME["fg_dim"],
                        font=(THEME["font_family"], 9))
        
        style.configure("TMenubutton", background=THEME["input_bg"], foreground=THEME["fg"],
                        arrowcolor=THEME["accent"], relief="flat", padding=6)
        style.map("TMenubutton", background=[("active", THEME["card_border"])])
        
        style.configure("Horizontal.TScale", background=THEME["card"], troughcolor=THEME["input_bg"],
                        sliderrelief="flat")

    def _load_config(self):
        import keymouse
        self.cfg = keymouse.load_config(CONFIG_PATH)

    def _build_ui(self):
        # Main layout container
        main = ttk.Frame(self, padding=20)
        main.pack(fill="both", expand=True)

        # Header Title
        title_box = ttk.Frame(main)
        title_box.pack(fill="x", pady=(0, 16))
        
        ttk.Label(title_box, text="KeyMouse Configuration", style="Title.TLabel").pack(anchor="w")
        ttk.Label(title_box, text="Control pointer using keyboard shortcuts with speed ramping",
                  style="Subtitle.TLabel").pack(anchor="w", pady=(2, 0))

        # Card 1: Key Bindings
        c1 = self._create_card(main, "Key Bindings")
        
        # Modifier Key
        f_mod = ttk.Frame(c1, style="Card.TFrame")
        f_mod.pack(fill="x", pady=4)
        ttk.Label(f_mod, text="Hold Key (Modifier):").pack(side="left")
        
        self.mod_var = tk.StringVar()
        for label, sym in MODIFIERS:
            if sym == self.cfg["modifier"]:
                self.mod_var.set(label)
                break
        else:
            self.mod_var.set(self.cfg["modifier"])
            
        mod_menu = ttk.OptionMenu(f_mod, self.mod_var, self.mod_var.get(),
                                  *[l for l, _ in MODIFIERS])
        mod_menu.pack(side="right")

        # Left / Right Click keys
        self.left_var = self._create_combo_row(c1, "Left Click / Tap:", self.cfg["left_click_key"], CLICK_KEYS)
        self.right_var = self._create_combo_row(c1, "Right Click:", self.cfg["right_click_key"], CLICK_KEYS)
        
        # Drag Key
        self.drag_var = self._create_combo_row(c1, "Dedicated Drag Key:", self.cfg.get("drag_key", ""), DRAG_KEYS)
        ttk.Label(c1, text="Separate drag key prevents accidental file opens when dragging", 
                  style="Dim.TLabel").pack(anchor="w", pady=(0, 4))

        # Card 2: Speed & Response
        c2 = self._create_card(main, "Speed & Responsiveness")
        
        self.speed_slider, self.speed_val = self._create_slider(
            c2, "Base Speed (px/tick):", self.cfg["base_speed"], 1, 30, "%d px")
        self.fast_slider, self.fast_val = self._create_slider(
            c2, "Fast Multiplier (+Shift):", self.cfg["fast_multiplier"], 1.5, 5.0, "%.1fx")
        self.accel_slider, self.accel_val = self._create_slider(
            c2, "Acceleration Rate:", self.cfg["accel_per_sec"], 20, 250, "%d px/s²")

        # Card 3: Daemon & System Status
        c3 = self._create_card(main, "Daemon Control & Startup")
        
        status_box = ttk.Frame(c3, style="Card.TFrame")
        status_box.pack(fill="x", pady=4)
        ttk.Label(status_box, text="Current State:").pack(side="left")
        self.lbl_status = ttk.Label(status_box, text="Checking...", font=(THEME["font_family"], 10, "bold"))
        self.lbl_status.pack(side="left", padx=8)

        # Autostart checkbox
        self.auto_var = tk.BooleanVar(value=autostart_enabled())
        chk = tk.Checkbutton(c3, text="Start automatically on login", variable=self.auto_var,
                             bg=THEME["card"], fg=THEME["fg"], activebackground=THEME["card"],
                             activeforeground=THEME["accent"], selectcolor=THEME["input_bg"],
                             highlightthickness=0, font=(THEME["font_family"], 10))
        chk.pack(anchor="w", pady=4)

        # Action Buttons Bottom
        btn_bar = ttk.Frame(main)
        btn_bar.pack(fill="x", side="bottom", pady=(16, 0))

        btn_save = tk.Button(btn_bar, text="Save & Apply", bg=THEME["accent"], fg="#0f172a",
                             activebackground=THEME["accent_hover"], activeforeground="#ffffff",
                             relief="flat", font=(THEME["font_family"], 10, "bold"),
                             padx=16, pady=6, cursor="hand2", command=self._on_save)
        btn_save.pack(side="right", padx=(8, 0))

        btn_toggle = tk.Button(btn_bar, text="Restart Daemon", bg=THEME["card"], fg=THEME["fg"],
                               activebackground=THEME["card_border"], activeforeground=THEME["fg"],
                               relief="flat", font=(THEME["font_family"], 10),
                               padx=12, pady=6, cursor="hand2", command=self._restart_daemon)
        btn_toggle.pack(side="right")

    def _create_card(self, parent, title):
        card = ttk.Frame(parent, style="Card.TFrame", padding=14)
        card.pack(fill="x", pady=(0, 12))
        ttk.Label(card, text=title, style="Header.TLabel").pack(anchor="w", pady=(0, 8))
        return card

    def _create_combo_row(self, parent, label_text, current_val, choices):
        row = ttk.Frame(parent, style="Card.TFrame")
        row.pack(fill="x", pady=4)
        ttk.Label(row, text=label_text).pack(side="left")
        
        var = tk.StringVar()
        for label, sym in choices:
            if sym == current_val:
                var.set(label)
                break
        else:
            var.set(current_val if current_val else choices[0][0])
            
        opt = ttk.OptionMenu(row, var, var.get(), *[l for l, _ in choices])
        opt.pack(side="right")
        return var

    def _create_slider(self, parent, label_text, default_val, min_val, max_val, fmt):
        row = ttk.Frame(parent, style="Card.TFrame")
        row.pack(fill="x", pady=4)
        
        lbl = ttk.Label(row, text=label_text)
        lbl.pack(side="left")
        
        val_lbl = ttk.Label(row, text=fmt % default_val, style="Dim.TLabel")
        val_lbl.pack(side="right", padx=(8, 0))
        
        def update_val(v):
            val_lbl.config(text=fmt % float(v))

        slider = ttk.Scale(row, from_=min_val, to=max_val, value=default_val,
                           command=update_val)
        slider.pack(side="right", fill="x", expand=True, padx=8)
        return slider, val_lbl

    def _refresh_status(self):
        if daemon_running():
            self.lbl_status.config(text="Active / Running", foreground=THEME["success"])
        else:
            self.lbl_status.config(text="Stopped", foreground=THEME["danger"])
        self.after(3000, self._refresh_status)

    def _restart_daemon(self):
        import keymouse
        subprocess.run(["pkill", "-f", "keymouse.py$"])
        exe = os.path.expanduser("~/.local/bin/keymouse")
        subprocess.Popen([exe], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        self.after(500, self._refresh_status)

    def _collect(self):
        # Resolve modifier
        mod_label = self.mod_var.get()
        mod_sym = "Control_R"
        for l, s in MODIFIERS:
            if l == mod_label:
                mod_sym = s
                break

        # Resolve Left Click
        left_label = self.left_var.get()
        left_sym = "Return"
        for l, s in CLICK_KEYS:
            if l == left_label:
                left_sym = s
                break

        # Resolve Right Click
        right_label = self.right_var.get()
        right_sym = "period"
        for l, s in CLICK_KEYS:
            if l == right_label:
                right_sym = s
                break

        # Resolve Drag Key
        drag_label = self.drag_var.get()
        drag_sym = ""
        for l, s in DRAG_KEYS:
            if l == drag_label:
                drag_sym = s
                break

        return {
            "modifier": mod_sym,
            "base_speed": int(round(self.speed_slider.get())),
            "max_speed": self.cfg.get("max_speed", 42),
            "accel_per_sec": int(round(self.accel_slider.get())),
            "tick_ms": self.cfg.get("tick_ms", 16),
            "fast_multiplier": round(self.fast_slider.get(), 1),
            "slow_multiplier": self.cfg.get("slow_multiplier", 0.25),
            "scroll_ms": self.cfg.get("scroll_ms", 120),
            "left_click_key": left_sym,
            "right_click_key": right_sym,
            "middle_click_key": self.cfg.get("middle_click_key", "slash"),
            "drag_key": drag_sym,
        }

    def _on_save(self):
        import keymouse
        values = self._collect()
        keymouse.save_config(CONFIG_PATH, values)
        set_autostart(self.auto_var.get())
        self._restart_daemon()


def main():
    app = ModernSettings()
    app.mainloop()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())