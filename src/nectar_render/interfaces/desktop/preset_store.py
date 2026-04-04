"""Helpers for desktop preset persistence and lookup."""

from __future__ import annotations

import logging
from pathlib import Path

from ...adapters.storage import load_json_file, save_json_file
from ...core.presets import BUILTIN_PRESET_NAMES, get_builtin_preset
from .state_mapping import style_to_state

_BUILTIN_PRESET_NAME_SET = set(BUILTIN_PRESET_NAMES)


def load_user_presets(
    presets_file: Path,
    *,
    log: logging.Logger | None = None,
) -> dict[str, dict[str, object]]:
    payload = load_json_file(presets_file, log=log)
    user_presets = payload.get("presets", {})
    if not isinstance(user_presets, dict):
        return {}

    cleaned: dict[str, dict[str, object]] = {}
    for name, state in user_presets.items():
        if isinstance(name, str) and isinstance(state, dict):
            cleaned[name] = state
    return cleaned


def list_preset_names(
    presets_file: Path,
    *,
    log: logging.Logger | None = None,
) -> list[str]:
    names = [f"{name} (built-in)" for name in BUILTIN_PRESET_NAMES]
    for name in sorted(load_user_presets(presets_file, log=log), key=str.casefold):
        if name not in _BUILTIN_PRESET_NAME_SET:
            names.append(name)
    return names


def save_user_preset(
    presets_file: Path,
    name: str,
    state: dict[str, object],
    *,
    log: logging.Logger | None = None,
) -> None:
    user_presets = load_user_presets(presets_file, log=log)
    user_presets[name] = state
    save_json_file(presets_file, {"presets": user_presets})


def resolve_preset_state(
    presets_file: Path,
    name: str,
    *,
    log: logging.Logger | None = None,
) -> dict[str, object] | None:
    builtin = get_builtin_preset(name)
    if builtin is not None:
        return style_to_state(builtin)
    return load_user_presets(presets_file, log=log).get(name)


__all__ = [
    "list_preset_names",
    "load_user_presets",
    "resolve_preset_state",
    "save_user_preset",
]
