"""Application state management: undo/redo, persistence, and presets."""

from __future__ import annotations

import json
import logging
import tkinter as tk
from pathlib import Path
from tkinter import messagebox

from ..config import PDF_COMPRESSION_PROFILES
from .presets import BUILTIN_PRESETS, BUILTIN_PRESET_NAMES, is_builtin_preset, get_builtin_preset

logger = logging.getLogger(__name__)

MAX_UNDO_HISTORY = 150
PERSIST_DEBOUNCE_MS = 500


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
        self._ensure_font_option: callable | None = None

    def set_tk_vars(
        self,
        tk_vars: dict[str, tk.Variable],
        status_var: tk.StringVar,
        ensure_font_option: callable,
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
                val = str(state.get(font_key, "")).strip()
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
                    var.set(_safe_int(raw, var.get()))
                elif isinstance(var, tk.DoubleVar):
                    var.set(_safe_float(raw, var.get()))
                elif isinstance(var, tk.StringVar):
                    if key == "pdf_compression_profile_var":
                        profile = str(raw).strip().lower()
                        if profile not in PDF_COMPRESSION_PROFILES:
                            profile = var.get()
                        var.set(profile)
                    elif key == "footer_align_var":
                        align = str(raw).strip()
                        var.set("Center" if align.lower() in {"center", "centre"} else "Right")
                    else:
                        var.set(str(raw))
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
        payload = _load_json_file(self.presets_file)
        user_presets = payload.get("presets", {})
        if not isinstance(user_presets, dict):
            user_presets = {}

        names: list[str] = []
        for name in BUILTIN_PRESET_NAMES:
            names.append(f"{name} (built-in)")
        for name in sorted(user_presets.keys()):
            if name not in BUILTIN_PRESETS:
                names.append(name)
        return names

    def save_preset(self, name: str, status_var: tk.StringVar | None = None) -> None:
        if not name:
            messagebox.showwarning("Preset", "Enter a preset name before saving.")
            return

        if is_builtin_preset(name):
            messagebox.showwarning("Preset", f"Cannot overwrite built-in preset: {name}")
            return

        payload = _load_json_file(self.presets_file)
        presets = payload.get("presets", {})
        if not isinstance(presets, dict):
            presets = {}
        presets[name] = self.capture_state()
        _save_json_file(self.presets_file, {"presets": presets})
        if status_var:
            status_var.set(f"Preset saved: {name}")

    def load_selected_preset(self, name: str, status_var: tk.StringVar | None = None) -> None:
        if not name:
            messagebox.showwarning("Preset", "Select or enter a preset name.")
            return

        # Check built-in presets first
        builtin = get_builtin_preset(name)
        if builtin is not None:
            self.apply_state(builtin, reset_redo=True)
            self.record_history_state()
            if status_var:
                status_var.set(f"Preset loaded: {name}")
            return

        # Then check user presets
        payload = _load_json_file(self.presets_file)
        presets = payload.get("presets", {})
        if not isinstance(presets, dict) or name not in presets:
            messagebox.showwarning("Preset", f"Preset not found: {name}")
            return

        state = presets.get(name)
        if isinstance(state, dict):
            self.apply_state(state, reset_redo=True)
            self.record_history_state()
            if status_var:
                status_var.set(f"Preset loaded: {name}")


# --- helpers (module-level) ---------------------------------------------------

def _safe_int(value: object, fallback: int) -> int:
    try:
        return int(value)
    except (ValueError, TypeError):
        return fallback


def _safe_float(value: object, fallback: float) -> float:
    try:
        return float(value)
    except (ValueError, TypeError):
        return fallback


def _save_json_file(path: Path, payload: dict[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")


def _load_json_file(path: Path) -> dict[str, object]:
    if not path.exists():
        return {}
    try:
        content = json.loads(path.read_text(encoding="utf-8"))
        if isinstance(content, dict):
            return content
    except json.JSONDecodeError:
        return {}
    return {}
