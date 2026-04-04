from __future__ import annotations

import tkinter as tk
from tkinter import ttk


def apply_ui_theme(root: tk.Tk, theme_name: str) -> None:
    style = ttk.Style(root)
    dark = theme_name.lower() == "dark"

    if dark:
        try:
            style.theme_use("clam")
        except tk.TclError:
            pass
        palette = {
            "bg": "#111827",
            "panel": "#1f2937",
            "fg": "#f9fafb",
            "muted": "#cbd5e1",
            "field": "#0f172a",
            "border": "#374151",
            "button": "#1f2937",
            "button_active": "#334155",
            "primary": "#2563eb",
            "primary_active": "#1d4ed8",
        }
    else:
        try:
            style.theme_use("clam")
        except tk.TclError:
            try:
                style.theme_use("default")
            except tk.TclError:
                pass
        palette = {
            "bg": "#f3f4f6",
            "panel": "#ffffff",
            "fg": "#111827",
            "muted": "#4b5563",
            "field": "#ffffff",
            "border": "#d1d5db",
            "button": "#e5e7eb",
            "button_active": "#d1d5db",
            "primary": "#2563eb",
            "primary_active": "#1d4ed8",
        }

    root.configure(bg=palette["bg"])
    style.configure(".", background=palette["bg"], foreground=palette["fg"])

    style.configure("TFrame", background=palette["bg"])
    style.configure("Card.TFrame", background=palette["panel"], relief="flat")

    style.configure("TLabel", background=palette["bg"], foreground=palette["fg"])
    style.configure(
        "Title.TLabel",
        background=palette["panel"],
        foreground=palette["fg"],
        font=("Segoe UI", 12, "bold"),
    )
    style.configure(
        "Muted.TLabel", background=palette["bg"], foreground=palette["muted"]
    )

    style.configure(
        "TLabelframe",
        background=palette["bg"],
        bordercolor=palette["border"],
        relief="solid",
    )
    style.configure(
        "TLabelframe.Label",
        background=palette["bg"],
        foreground=palette["fg"],
        font=("Segoe UI", 10, "bold"),
    )

    style.configure(
        "TButton",
        padding=7,
        background=palette["button"],
        foreground=palette["fg"],
        relief="flat",
        borderwidth=1,
    )
    style.map("TButton", background=[("active", palette["button_active"])])

    style.configure(
        "Primary.TButton",
        padding=7,
        background=palette["primary"],
        foreground="#ffffff",
        relief="flat",
        borderwidth=1,
    )
    style.map("Primary.TButton", background=[("active", palette["primary_active"])])

    style.configure("TCheckbutton", background=palette["bg"], foreground=palette["fg"])
    style.configure(
        "TEntry", fieldbackground=palette["field"], foreground=palette["fg"]
    )
    style.configure(
        "TSpinbox", fieldbackground=palette["field"], foreground=palette["fg"]
    )
    style.configure(
        "TCombobox", fieldbackground=palette["field"], foreground=palette["fg"]
    )

    style.configure("TScale", background=palette["bg"])
