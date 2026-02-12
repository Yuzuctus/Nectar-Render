"""Reusable Tkinter widgets for the MD-TO-PDF application."""

from __future__ import annotations

import logging
import tkinter as tk
from tkinter import ttk

logger = logging.getLogger(__name__)


def normalized_query(text: str) -> str:
    """Collapse whitespace and casefold *text* for fuzzy comparison."""
    return "".join((text or "").split()).casefold()


def fuzzy_match(query: str, candidate: str) -> bool:
    """Return True if *query* fuzzy-matches *candidate*."""
    q = normalized_query(query)
    if not q:
        return True

    c = (candidate or "").casefold()
    if q in c.replace(" ", ""):
        return True

    pos = 0
    for ch in q:
        found = c.find(ch, pos)
        if found < 0:
            return False
        pos = found + 1
    return True


def filter_font_families(query: str, families: list[str], limit: int = 300) -> list[str]:
    """Return the subset of *families* that fuzzy-match *query*."""
    if not query.strip():
        return list(families)
    matched = [name for name in families if fuzzy_match(query, name)]
    return matched[:limit]


class FontAutocomplete:
    """Entry + persistent Listbox popup with fuzzy filtering.

    Key design decisions (hardened for Windows):
    - The popup Toplevel is created once and hidden/shown via withdraw/deiconify
      instead of being destroyed/recreated.  This avoids Toplevel recreation
      issues and event-binding fragility.
    - Click-outside detection uses **coordinate hit-testing** instead of
      ``winfo_containing`` which is unreliable for ``wm_overrideredirect``
      windows on Windows.
    - The button toggles the popup (open / close).
    """

    FILTER_DEBOUNCE_MS = 80

    def __init__(
        self,
        root: tk.Tk,
        parent: ttk.Frame,
        textvariable: tk.StringVar,
        values: list[str],
        width: int,
        allow_custom_values: bool = True,
        on_commit: callable | None = None,
    ) -> None:
        self.root = root
        self.var = textvariable
        self._all_values: list[str] = list(values)
        self._allow_custom_values = allow_custom_values
        current = (self.var.get() or "").strip()
        self._last_valid_value = current if current in self._all_values else (self._all_values[0] if self._all_values else "")
        if self._last_valid_value:
            self.var.set(self._last_valid_value)
        self._job: str | None = None
        self._is_open = False
        self._on_commit = on_commit

        # --- visible widgets --------------------------------------------------
        self.frame = ttk.Frame(parent)
        self.entry = ttk.Entry(self.frame, textvariable=self.var, width=width)
        self.entry.pack(side=tk.LEFT, fill=tk.X, expand=True)
        self.button = ttk.Button(self.frame, text="\u25bc", width=2, command=self._toggle_popup)
        self.button.pack(side=tk.LEFT, padx=(6, 0))

        # --- persistent popup (hidden) ----------------------------------------
        self.popup = tk.Toplevel(self.root)
        self.popup.wm_overrideredirect(True)
        self.popup.wm_transient(self.root)
        self.popup.withdraw()

        self.listbox = tk.Listbox(self.popup, exportselection=False, height=12)
        self.scrollbar = ttk.Scrollbar(self.popup, orient=tk.VERTICAL, command=self.listbox.yview)
        self.listbox.configure(yscrollcommand=self.scrollbar.set)
        self.listbox.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)
        self.scrollbar.pack(side=tk.RIGHT, fill=tk.Y)

        # --- bindings ---------------------------------------------------------
        self.listbox.bind("<ButtonRelease-1>", self._on_click_select)
        self.listbox.bind("<Return>", self._on_return)
        self.listbox.bind("<Escape>", self._on_escape)

        self.entry.bind("<KeyRelease>", self._on_keyrelease)
        self.entry.bind("<Down>", self._on_down)
        self.entry.bind("<Up>", self._on_up)
        self.entry.bind("<Return>", self._on_return)
        self.entry.bind("<Escape>", self._on_escape)
        self.entry.bind("<FocusOut>", self._on_focus_out)

        self.root.bind_all("<Button-1>", self._on_global_click, add="+")

    # --- public API -----------------------------------------------------------

    def set_values(self, values: list[str]) -> None:
        self._all_values = list(values)
        if self._is_open:
            self._refresh_listbox()

    # --- popup visibility (withdraw / deiconify, never destroy) ---------------

    def _show_popup(self) -> None:
        self._position_popup()
        self.popup.deiconify()
        self.popup.lift()
        self._is_open = True

    def _hide_popup(self) -> None:
        if not self._is_open:
            return
        self.popup.withdraw()
        self._is_open = False

    def _position_popup(self) -> None:
        try:
            self.frame.update_idletasks()
            x = self.frame.winfo_rootx()
            y = self.frame.winfo_rooty() + self.frame.winfo_height()
            width = max(self.frame.winfo_width(), 260)
            height = 220

            screen_w = self.root.winfo_screenwidth()
            screen_h = self.root.winfo_screenheight()
            x = max(0, min(x, screen_w - width))
            y = max(0, min(y, screen_h - height))

            self.popup.geometry(f"{width}x{height}+{x}+{y}")
        except tk.TclError:
            pass

    # --- listbox content ------------------------------------------------------

    def _refresh_listbox(self, show_all: bool = False) -> bool:
        query = self.var.get()
        if show_all:
            matches = list(self._all_values)
        else:
            matches = filter_font_families(query, self._all_values)
        try:
            self.listbox.delete(0, tk.END)
            for name in matches:
                self.listbox.insert(tk.END, name)
            if matches:
                self.listbox.selection_clear(0, tk.END)
                self.listbox.selection_set(0)
                self.listbox.activate(0)
        except tk.TclError:
            pass
        return bool(matches)

    # --- toggle button --------------------------------------------------------

    def _toggle_popup(self) -> None:
        if self._is_open:
            self._hide_popup()
        else:
            self._refresh_listbox(show_all=True)
            self._show_popup()
        self.entry.focus_set()

    # --- listbox navigation ---------------------------------------------------

    def _move_selection(self, delta: int) -> None:
        try:
            size = self.listbox.size()
            if size <= 0:
                return
            selection = self.listbox.curselection()
            index = int(selection[0]) if selection else 0
            index = max(0, min(size - 1, index + delta))
            self.listbox.selection_clear(0, tk.END)
            self.listbox.selection_set(index)
            self.listbox.activate(index)
            self.listbox.see(index)
        except tk.TclError:
            pass

    def _selected_value(self) -> str | None:
        try:
            selection = self.listbox.curselection()
            if not selection:
                return None
            return str(self.listbox.get(selection[0]))
        except tk.TclError:
            return None

    # --- commit selection -----------------------------------------------------

    def _commit(self, value: str) -> None:
        self.var.set(value)
        self._last_valid_value = value
        self._hide_popup()
        if self._on_commit:
            try:
                self._on_commit(value)
            except Exception:
                logger.exception("Error applying font selection")
        try:
            self.entry.focus_set()
            self.entry.icursor(tk.END)
        except tk.TclError:
            pass

    # --- event handlers -------------------------------------------------------

    def _on_keyrelease(self, event: tk.Event[tk.Misc]) -> None:
        if event.keysym in {"Up", "Down", "Return", "Escape", "Tab"}:
            return
        if self._job:
            try:
                self.root.after_cancel(self._job)
            except tk.TclError:
                pass
            self._job = None
        self._job = self.root.after(self.FILTER_DEBOUNCE_MS, self._do_filter)

    def _do_filter(self) -> None:
        self._job = None
        query = self.var.get().strip()
        if not query:
            self._hide_popup()
            return
        has_matches = self._refresh_listbox(show_all=False)
        if has_matches:
            self._show_popup()
        else:
            self._hide_popup()

    def _on_down(self, _event: tk.Event[tk.Misc]) -> str:
        if not self._is_open:
            self._refresh_listbox(show_all=not self.var.get().strip())
            self._show_popup()
        self._move_selection(+1)
        return "break"

    def _on_up(self, _event: tk.Event[tk.Misc]) -> str:
        self._move_selection(-1)
        return "break"

    def _on_return(self, _event: tk.Event[tk.Misc] | None = None) -> str:
        selected = self._selected_value()
        if selected:
            self._commit(selected)
            return "break"
        typed = self.var.get().strip()
        if typed:
            if not self._allow_custom_values and typed not in self._all_values:
                if self._last_valid_value:
                    self.var.set(self._last_valid_value)
                self._hide_popup()
                return "break"
            self._commit(typed)
        return "break"

    def _on_escape(self, _event: tk.Event[tk.Misc] | None = None) -> str:
        self._hide_popup()
        try:
            self.entry.focus_set()
        except tk.TclError:
            pass
        return "break"

    def _on_click_select(self, _event: tk.Event[tk.Misc]) -> None:
        selected = self._selected_value()
        if selected:
            self._commit(selected)

    def _on_focus_out(self, _event: tk.Event[tk.Misc]) -> None:
        if not self._allow_custom_values:
            typed = self.var.get().strip()
            if typed and typed not in self._all_values:
                self.var.set(self._last_valid_value)
            return

        typed = self.var.get().strip()
        if typed and typed not in self._all_values:
            self._all_values.append(typed)
            self._all_values.sort(key=str.casefold)

    # --- click-outside detection (coordinate-based, NOT winfo_containing) -----

    @staticmethod
    def _coords_inside_widget(x: int, y: int, widget: tk.Misc) -> bool:
        """Return True if screen coordinates (x, y) fall within *widget*."""
        try:
            wx = widget.winfo_rootx()
            wy = widget.winfo_rooty()
            ww = widget.winfo_width()
            wh = widget.winfo_height()
            return wx <= x <= wx + ww and wy <= y <= wy + wh
        except tk.TclError:
            return False

    def _on_global_click(self, event: tk.Event[tk.Misc]) -> None:
        if not self._is_open:
            return
        try:
            x = event.x_root
            y = event.y_root
        except (tk.TclError, AttributeError):
            return

        if self._coords_inside_widget(x, y, self.entry):
            return
        if self._coords_inside_widget(x, y, self.button):
            return
        if self._coords_inside_widget(x, y, self.popup):
            return

        self._hide_popup()
