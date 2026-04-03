from __future__ import annotations

import re

from pygments.formatters import HtmlFormatter
from pygments.styles import get_all_styles, get_style_by_name
from pygments.util import ClassNotFound


_STYLE_ALIASES = {
    "light": "default",
    "dark": "native",
    "monokai": "monokai",
}


def _parse_color_to_rgb(color: str) -> tuple[int, int, int] | None:
    normalized = (color or "").strip().lower()
    if not normalized:
        return None

    if normalized.startswith("#"):
        hex_value = normalized[1:]
        if len(hex_value) == 3:
            hex_value = "".join(char * 2 for char in hex_value)
        if len(hex_value) in (6, 8):
            hex_value = hex_value[:6]
            try:
                red = int(hex_value[0:2], 16)
                green = int(hex_value[2:4], 16)
                blue = int(hex_value[4:6], 16)
            except ValueError:
                return None
            return (red, green, blue)
        return None

    match = re.fullmatch(
        r"rgba?\((\d{1,3}),\s*(\d{1,3}),\s*(\d{1,3})(?:,\s*(?:\d+(?:\.\d+)?|\.\d+))?\)",
        normalized,
    )
    if not match:
        return None

    red, green, blue = (int(component) for component in match.groups())
    if red > 255 or green > 255 or blue > 255:
        return None
    return (red, green, blue)


def _is_dark_style(style_name: str) -> bool:
    resolved = resolve_style_name(style_name)
    style = get_style_by_name(resolved)
    rgb = _parse_color_to_rgb(style.background_color or "")
    if rgb is None:
        return False
    red, green, blue = rgb
    luminance = (0.299 * red + 0.587 * green + 0.114 * blue) / 255
    return luminance < 0.5


def list_available_styles() -> list[str]:
    styles = sorted(set(get_all_styles()))
    light_styles = [
        style_name for style_name in styles if not _is_dark_style(style_name)
    ]
    dark_styles = [style_name for style_name in styles if _is_dark_style(style_name)]

    light_aliases = [
        alias
        for alias in _STYLE_ALIASES
        if alias not in styles and not _is_dark_style(alias)
    ]
    dark_aliases = [
        alias
        for alias in _STYLE_ALIASES
        if alias not in styles and _is_dark_style(alias)
    ]

    return light_aliases + light_styles + dark_aliases + dark_styles


def resolve_style_name(style_name: str) -> str:
    normalized = (style_name or "default").strip().lower()
    return _STYLE_ALIASES.get(normalized, normalized)


def build_pygments_css(style_name: str) -> str:
    try:
        formatter = HtmlFormatter(style=resolve_style_name(style_name))
    except ClassNotFound:
        formatter = HtmlFormatter(style="default")
    pygments_css = formatter.get_style_defs(".codehilite")
    return f"{pygments_css}\n.codehilite pre {{ background: inherit; }}"
