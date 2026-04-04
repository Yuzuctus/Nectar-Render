"""Application state management: undo/redo, persistence, and presets."""

from __future__ import annotations

import logging
import tkinter as tk
from collections.abc import Callable
from pathlib import Path
from tkinter import messagebox

from ...adapters.storage import load_json_file, save_json_file
from ...config import OUTPUT_FORMATS, PAGE_SIZES, PDF_COMPRESSION_PROFILES, UI_THEMES
from ...core.presets import is_builtin_preset
from ...utils.converters import safe_int, safe_float
from .preset_store import list_preset_names, resolve_preset_state, save_user_preset
from .state_mapping import (
    is_style_state_key,
    normalize_style_state_value,
    sanitize_text_value,
)

logger = logging.getLogger(__name__)

MAX_UNDO_HISTORY = 150
PERSIST_DEBOUNCE_MS = 500
_VALID_OUTPUT_FORMATS = {name.casefold(): name for name in OUTPUT_FORMATS}
_VALID_PAGE_SIZES = {name.casefold(): name for name in PAGE_SIZES}
_VALID_UI_THEMES = {name.casefold(): name for name in UI_THEMES}


def _sanitize_text(value: object) -> str:
    return sanitize_text_value(value)


def _clamp_int_value(key: str, value: object, fallback: int) -> int:
    if is_style_state_key(key):
        normalized = normalize_style_state_value(key, value, fallback)
        return safe_int(normalized, fallback)
    return safe_int(value, fallback)


def _clamp_float_value(key: str, value: object, fallback: float) -> float:
    if is_style_state_key(key):
        normalized = normalize_style_state_value(key, value, fallback)
        return safe_float(normalized, fallback)
    return safe_float(value, fallback)


def _normalize_string_state(key: str, value: object, fallback: str) -> str:
    cleaned = _sanitize_text(value)
    if key == "pdf_compression_profile_var":
        profile = cleaned.casefold()
        return profile if profile in PDF_COMPRESSION_PROFILES else fallback
    if key == "footer_align_var":
        return "Center" if cleaned.casefold() in {"center", "centre"} else "Right"
    if key == "format_var":
        return _VALID_OUTPUT_FORMATS.get(cleaned.casefold(), fallback)
    if key == "page_size_var":
        return _VALID_PAGE_SIZES.get(cleaned.casefold(), fallback)
    if key == "ui_theme_var":
        return _VALID_UI_THEMES.get(cleaned.casefold(), fallback)
    if is_style_state_key(key):
        normalized = normalize_style_state_value(key, value, fallback)
        return str(normalized)
    return cleaned or fallback


class StateManager:
    """Manages capture/apply of Tk variable state, undo/redo, persistence, and presets."""

    def __init__(
        self,
        root: tk.Tk,
        app_dir: Path,
        presets_filename: str,
        state_filename: str,
    ) -> None:
        self.root = root
        self.app_dir = app_dir
        self.presets_file = app_dir / presets_filename
        self.state_file = app_dir / state_filename

        self.history_lock = False
        self.undo_stack: list[dict[str, object]] = []
        self.redo_stack: list[dict[str, object]] = []
        self.persist_job: str | None = None

        # Will be set by the app after Tk variables are created
        self._tk_vars: dict[str, tk.Variable] = {}
        self._status_var: tk.StringVar | None = None
        self._ensure_font_option: Callable[..., object] | None = None

    def set_tk_vars(
        self,
        tk_vars: dict[str, tk.Variable],
        status_var: tk.StringVar,
        ensure_font_option: Callable[..., object],
    ) -> None:
        """Register the app's Tk variables for state capture/apply."""
        self._tk_vars = tk_vars
        self._status_var = status_var
        self._ensure_font_option = ensure_font_option

    # --- state capture / apply ------------------------------------------------

    def capture_state(self) -> dict[str, object]:
        """Snapshot all registered Tk variables into a plain dict."""
        return {key: var.get() for key, var in self._tk_vars.items()}

    def apply_state(self, state: dict[str, object], reset_redo: bool = False) -> None:
        """Restore Tk variables from *state* dict."""
        # Ensure custom fonts are available in comboboxes
        if self._ensure_font_option:
            for font_key in ("body_font_var", "heading_font_var", "code_font_var"):
                val = _sanitize_text(state.get(font_key, ""))
                if val:
                    self._ensure_font_option(val)

        self.history_lock = True
        try:
            for key, var in self._tk_vars.items():
                if key not in state:
                    continue
                raw = state[key]
                if isinstance(var, tk.BooleanVar):
                    var.set(bool(raw))
                elif isinstance(var, tk.IntVar):
                    var.set(_clamp_int_value(key, raw, var.get()))
                elif isinstance(var, tk.DoubleVar):
                    var.set(_clamp_float_value(key, raw, var.get()))
                elif isinstance(var, tk.StringVar):
                    var.set(_normalize_string_state(key, raw, var.get()))
        finally:
            self.history_lock = False

        if reset_redo:
            self.redo_stack = []
        self.schedule_persist_state()

    # --- undo / redo ----------------------------------------------------------

    def initialize_history(self) -> None:
        """Take the first snapshot for undo history."""
        snapshot = self.capture_state()
        self.undo_stack = [snapshot]
        self.redo_stack = []

    def record_history_state(self) -> None:
        """Record current state into the undo stack (if changed)."""
        if self.history_lock:
            return
        snapshot = self.capture_state()
        if self.undo_stack and snapshot == self.undo_stack[-1]:
            return
        self.undo_stack.append(snapshot)
        if len(self.undo_stack) > MAX_UNDO_HISTORY:
            self.undo_stack = self.undo_stack[-MAX_UNDO_HISTORY:]
        self.redo_stack = []

    def undo_action(self, _event: tk.Event[tk.Misc] | None = None) -> str:
        if len(self.undo_stack) <= 1:
            if self._status_var:
                self._status_var.set("Nothing to undo")
            return "break"
        current = self.undo_stack.pop()
        self.redo_stack.append(current)
        previous = self.undo_stack[-1]
        self.apply_state(previous)
        if self._status_var:
            self._status_var.set("Undo (Ctrl+Z)")
        return "break"

    def redo_action(self, _event: tk.Event[tk.Misc] | None = None) -> str:
        if not self.redo_stack:
            if self._status_var:
                self._status_var.set("Nothing to redo")
            return "break"
        restored = self.redo_stack.pop()
        self.undo_stack.append(restored)
        self.apply_state(restored)
        if self._status_var:
            self._status_var.set("Redo (Ctrl+Alt+Z)")
        return "break"

    # --- persistence ----------------------------------------------------------

    def schedule_persist_state(self) -> None:
        """Debounced save of current state to disk."""
        if self.persist_job:
            self.root.after_cancel(self.persist_job)
        self.persist_job = self.root.after(PERSIST_DEBOUNCE_MS, self.save_last_state)

    def save_last_state(self) -> None:
        payload = {"state": self.capture_state()}
        _save_json_file(self.state_file, payload)

    def load_last_state(self) -> None:
        payload = _load_json_file(self.state_file)
        state = payload.get("state")
        if isinstance(state, dict):
            self.apply_state(state)

    # --- presets --------------------------------------------------------------

    def load_presets(self) -> list[str]:
        """Return sorted list of all preset names (built-in + user)."""
        return list_preset_names(self.presets_file, log=logger)

    def save_preset(self, name: str, status_var: tk.StringVar | None = None) -> None:
        if not name:
            messagebox.showwarning("Preset", "Enter a preset name before saving.")
            return

        if is_builtin_preset(name):
            messagebox.showwarning(
                "Preset", f"Cannot overwrite built-in preset: {name}"
            )
            return

        save_user_preset(
            self.presets_file,
            name,
            self.capture_state(),
            log=logger,
        )
        if status_var:
            status_var.set(f"Preset saved: {name}")

    def load_selected_preset(
        self, name: str, status_var: tk.StringVar | None = None
    ) -> None:
        if not name:
            messagebox.showwarning("Preset", "Select or enter a preset name.")
            return

        state = resolve_preset_state(self.presets_file, name, log=logger)
        if state is None:
            messagebox.showwarning("Preset", f"Preset not found: {name}")
            return

        self.apply_state(state, reset_redo=True)
        self.record_history_state()
        if status_var:
            status_var.set(f"Preset loaded: {name}")


# --- helpers (module-level) ---------------------------------------------------


def _save_json_file(path: Path, payload: dict[str, object]) -> None:
    save_json_file(path, payload)


def _load_json_file(path: Path) -> dict[str, object]:
    return load_json_file(path, log=logger)
