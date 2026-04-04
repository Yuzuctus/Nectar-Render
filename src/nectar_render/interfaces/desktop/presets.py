"""Desktop preset catalog exposed as Tk-state dictionaries."""

from __future__ import annotations

from ...core.presets import BUILTIN_PRESET_STYLES
from .state_mapping import style_to_state

BUILTIN_PRESETS: dict[str, dict[str, object]] = {
    name: style_to_state(style) for name, style in BUILTIN_PRESET_STYLES.items()
}
BUILTIN_PRESET_NAMES: list[str] = sorted(BUILTIN_PRESETS)


def _clean_preset_name(name: str) -> str:
    return name.removesuffix(" (built-in)").strip()


def is_builtin_preset(name: str) -> bool:
    return _clean_preset_name(name) in BUILTIN_PRESETS


def get_builtin_preset(name: str) -> dict[str, object] | None:
    preset = BUILTIN_PRESETS.get(_clean_preset_name(name))
    if preset is None:
        return None
    return dict(preset)


__all__ = [
    "BUILTIN_PRESETS",
    "BUILTIN_PRESET_NAMES",
    "is_builtin_preset",
    "get_builtin_preset",
]
