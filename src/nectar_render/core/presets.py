from __future__ import annotations

import json
from dataclasses import asdict
from pathlib import Path

from .styles import StyleOptions, style_from_option_mapping

_PRESET_DATA_DIR = Path(__file__).parent / "preset_data"


def _load_preset_jsons() -> dict[str, dict[str, object]]:
    diffs: dict[str, dict[str, object]] = {}
    if not _PRESET_DATA_DIR.is_dir():
        return diffs
    for path in sorted(_PRESET_DATA_DIR.glob("*.json")):
        if path.name.startswith("_"):
            continue
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, OSError):
            continue
        name = data.get("name") or path.stem
        style = data.get("style")
        if isinstance(style, dict):
            diffs[name] = style
    return diffs


_LOADED_PRESET_DIFFS: dict[str, dict[str, object]] = _load_preset_jsons()

BUILTIN_PRESET_STYLES: dict[str, StyleOptions] = {
    name: style_from_option_mapping(values)
    for name, values in _LOADED_PRESET_DIFFS.items()
}
BUILTIN_PRESET_NAMES: list[str] = sorted(BUILTIN_PRESET_STYLES)

BUILTIN_PRESET_AS_DICTS: dict[str, dict[str, object]] = {
    name: {k: v for k, v in _LOADED_PRESET_DIFFS.get(name, {}).items()}
    for name in BUILTIN_PRESET_NAMES
}


def _clean_preset_name(name: str) -> str:
    return name.removesuffix(" (built-in)").strip()


def is_builtin_preset(name: str) -> bool:
    return _clean_preset_name(name) in BUILTIN_PRESET_STYLES


def get_builtin_preset(name: str) -> StyleOptions | None:
    preset = BUILTIN_PRESET_STYLES.get(_clean_preset_name(name))
    if preset is None:
        return None
    return StyleOptions(**asdict(preset))


def get_builtin_preset_raw(name: str) -> dict[str, object] | None:
    return _LOADED_PRESET_DIFFS.get(_clean_preset_name(name))
