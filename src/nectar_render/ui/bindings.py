from __future__ import annotations

import tkinter as tk
from collections.abc import Callable, Iterable

from .state_manager import StateManager


class UiBindingManager:
    """Tracks Tk variable traces and root bindings for cleanup."""

    def __init__(self, root: tk.Misc) -> None:
        self.root = root
        self._root_bindings: list[tuple[str, str]] = []
        self._variable_traces: list[tuple[tk.Variable, str]] = []

    def bind_root(
        self,
        sequence: str,
        callback: Callable[[tk.Event[tk.Misc]], object] | Callable[..., object],
        *,
        add: str = "+",
    ) -> None:
        func_id = self.root.bind(sequence, callback, add=add)
        if isinstance(func_id, str) and func_id:
            self._root_bindings.append((sequence, func_id))

    def trace_variables(
        self,
        variables: Iterable[tk.Variable],
        callback: Callable[..., object],
    ) -> None:
        for variable in variables:
            trace_id = variable.trace_add("write", callback)
            self._variable_traces.append((variable, trace_id))

    def dispose(self) -> None:
        for variable, trace_id in self._variable_traces:
            try:
                variable.trace_remove("write", trace_id)
            except (tk.TclError, ValueError):
                pass
        self._variable_traces.clear()

        for sequence, func_id in self._root_bindings:
            try:
                self.root.unbind(sequence, func_id)
            except tk.TclError:
                pass
        self._root_bindings.clear()


class OptionChangeController:
    """Debounces history and persistence updates after Tk variable changes."""

    def __init__(
        self, root: tk.Misc, state_manager: StateManager, debounce_ms: int
    ) -> None:
        self.root = root
        self.state_manager = state_manager
        self.debounce_ms = debounce_ms
        self._job: str | None = None

    def on_option_changed(self, *_args: object) -> None:
        if self._job is not None:
            try:
                self.root.after_cancel(self._job)
            except tk.TclError:
                pass
        self._job = self.root.after(self.debounce_ms, self.flush)

    def flush(self) -> None:
        self._job = None
        self.state_manager.record_history_state()
        self.state_manager.schedule_persist_state()

    def dispose(self) -> None:
        if self._job is None:
            return
        try:
            self.root.after_cancel(self._job)
        except tk.TclError:
            pass
        self._job = None
