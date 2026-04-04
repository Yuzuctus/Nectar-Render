from __future__ import annotations

from collections.abc import Mapping

from ...adapters.rendering.highlight import list_available_styles
from ...core.styles import (
    STYLE_OPTION_FLOAT_BOUNDS,
    STYLE_OPTION_INT_BOUNDS,
    StyleOptions,
    sanitize_text_value,
    style_defaults,
    style_from_option_mapping,
)
from ...utils.converters import safe_float, safe_int


_STYLE_NUMERIC_SPECS: dict[str, tuple[str, str, float, float]] = {
    "body_size_var": ("body_font_size", "int", 8, 24),
    "code_size_var": ("code_font_size", "int", 8, 24),
    "line_height_var": ("line_height", "float", 1.0, 2.4),
    "code_line_height_var": ("code_line_height", "float", 1.0, 2.4),
    "margin_top_var": ("margin_top_mm", "float", 0.0, 100.0),
    "margin_right_var": ("margin_right_mm", "float", 0.0, 100.0),
    "margin_bottom_var": ("margin_bottom_mm", "float", 0.0, 100.0),
    "margin_left_var": ("margin_left_mm", "float", 0.0, 100.0),
    "footnote_size_var": ("footnote_font_size", "float", 7.0, 16.0),
    "table_pad_y_var": ("table_cell_padding_y_px", "int", 0, 30),
    "table_pad_x_var": ("table_cell_padding_x_px", "int", 0, 40),
    "image_scale_var": ("image_scale", "float", 40.0, 100.0),
}
for _level in range(1, 7):
    _STYLE_NUMERIC_SPECS[f"heading_h{_level}_size_var"] = (
        f"heading_h{_level}_size_px",
        "int",
        8,
        96,
    )

STYLE_STATE_TO_OPTION: dict[str, str] = {
    "body_font_var": "body_font",
    "heading_font_var": "heading_font",
    "heading_color_var": "heading_color",
    "code_font_var": "code_font",
    "code_theme_var": "code_theme",
    "footer_text_var": "footer_text",
    "footer_color_var": "footer_color",
    "footer_align_var": "footer_align",
    "include_notes_var": "include_footnotes",
    "footnote_text_color_var": "footnote_text_color",
    "footnote_marker_color_var": "footnote_marker_color",
    "table_stripes_var": "table_row_stripes",
    "table_odd_color_var": "table_row_odd_color",
    "table_even_color_var": "table_row_even_color",
    "show_horizontal_rules_var": "show_horizontal_rules",
}
STYLE_STATE_TO_OPTION.update(
    {
        state_key: option_name
        for state_key, (option_name, *_rest) in _STYLE_NUMERIC_SPECS.items()
    }
)
for _level in range(1, 7):
    STYLE_STATE_TO_OPTION[f"heading_h{_level}_color_var"] = f"heading_h{_level}_color"

STYLE_STATE_INT_BOUNDS = {
    state_key: (int(minimum), int(maximum))
    for state_key, (
        _option_name,
        kind,
        minimum,
        maximum,
    ) in _STYLE_NUMERIC_SPECS.items()
    if kind == "int"
}
STYLE_STATE_FLOAT_BOUNDS = {
    state_key: (float(minimum), float(maximum))
    for state_key, (
        _option_name,
        kind,
        minimum,
        maximum,
    ) in _STYLE_NUMERIC_SPECS.items()
    if kind == "float"
}
_STYLE_BOOLEAN_STATE_KEYS = {
    "include_notes_var",
    "table_stripes_var",
    "show_horizontal_rules_var",
}


def style_state_keys() -> tuple[str, ...]:
    return tuple(STYLE_STATE_TO_OPTION)


def is_style_state_key(key: str) -> bool:
    return key in STYLE_STATE_TO_OPTION


def _normalize_footer_align_state(value: object, fallback: object) -> str:
    cleaned = sanitize_text_value(value).casefold()
    if cleaned in {"center", "centre"}:
        return "Center"
    fallback_clean = sanitize_text_value(fallback).casefold()
    return "Center" if fallback_clean == "center" else "Right"


def _normalize_code_theme_state(value: object, fallback: object) -> str:
    cleaned = sanitize_text_value(value)
    valid_code_themes = {name.casefold(): name for name in list_available_styles()}
    return valid_code_themes.get(cleaned.casefold(), str(fallback))


def normalize_style_state_value(
    state_key: str, value: object, fallback: object
) -> object:
    if state_key in STYLE_STATE_INT_BOUNDS:
        minimum, maximum = STYLE_STATE_INT_BOUNDS[state_key]
        parsed = safe_int(value, safe_int(fallback, minimum))
        return max(minimum, min(maximum, parsed))

    if state_key in STYLE_STATE_FLOAT_BOUNDS:
        minimum, maximum = STYLE_STATE_FLOAT_BOUNDS[state_key]
        parsed = safe_float(value, safe_float(fallback, minimum))
        return max(minimum, min(maximum, parsed))

    if state_key == "footer_align_var":
        return _normalize_footer_align_state(value, fallback)

    if state_key == "code_theme_var":
        return _normalize_code_theme_state(value, fallback)

    if state_key in _STYLE_BOOLEAN_STATE_KEYS:
        if isinstance(value, str):
            return sanitize_text_value(value).casefold() not in {
                "",
                "0",
                "false",
                "no",
                "off",
            }
        return bool(value)

    cleaned = sanitize_text_value(value)
    fallback_text = sanitize_text_value(fallback)
    return cleaned or fallback_text


def _style_option_to_state_value(option_name: str, value: object) -> object:
    if option_name == "footer_align":
        return (
            "Center" if sanitize_text_value(value).casefold() == "center" else "Right"
        )
    if option_name == "image_scale":
        return round(safe_float(value, 0.9) * 100.0, 1)
    return value


def _style_option_from_state_value(option_name: str, value: object) -> object:
    if option_name == "footer_align":
        return (
            "center" if sanitize_text_value(value).casefold() == "center" else "right"
        )
    if option_name == "image_scale":
        return safe_float(value, 90.0) / 100.0
    return value


def style_from_state(
    state: Mapping[str, object],
    base_style: StyleOptions | None = None,
) -> StyleOptions:
    style_kwargs = style_defaults(base_style)

    for state_key, option_name in STYLE_STATE_TO_OPTION.items():
        if state_key not in state:
            continue
        fallback = _style_option_to_state_value(option_name, style_kwargs[option_name])
        normalized_state_value = normalize_style_state_value(
            state_key,
            state[state_key],
            fallback,
        )
        style_kwargs[option_name] = _style_option_from_state_value(
            option_name,
            normalized_state_value,
        )

    return StyleOptions(**style_kwargs)


def style_to_state(style: StyleOptions | Mapping[str, object]) -> dict[str, object]:
    if isinstance(style, StyleOptions):
        option_values = style_defaults(style)
    else:
        option_values = style_defaults(style_from_option_mapping(style))

    state: dict[str, object] = {}
    for state_key, option_name in STYLE_STATE_TO_OPTION.items():
        state[state_key] = _style_option_to_state_value(
            option_name,
            option_values[option_name],
        )
    return state


__all__ = [
    "STYLE_OPTION_FLOAT_BOUNDS",
    "STYLE_OPTION_INT_BOUNDS",
    "STYLE_STATE_FLOAT_BOUNDS",
    "STYLE_STATE_INT_BOUNDS",
    "STYLE_STATE_TO_OPTION",
    "is_style_state_key",
    "normalize_style_state_value",
    "sanitize_text_value",
    "style_from_state",
    "style_state_keys",
    "style_to_state",
]
