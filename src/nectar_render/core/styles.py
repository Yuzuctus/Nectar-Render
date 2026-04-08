from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass, field, fields


OUTPUT_FORMATS = ("PDF", "HTML", "PDF+HTML")
PAGE_SIZES = ("A4", "Letter", "Legal", "A3", "A5")
UI_THEMES = ("Light", "Dark")
PDF_COMPRESSION_PROFILES = ("balanced", "max")

DEFAULT_BODY_FONT = "Segoe UI"
DEFAULT_CODE_FONT = "Consolas"
DEFAULT_HEADING_COLOR = "#1f2937"
FALLBACK_FONTS = (
    "Segoe UI",
    "Calibri",
    "Arial",
    "Consolas",
    "Courier New",
    "Times New Roman",
)


@dataclass(slots=True)
class StyleOptions:
    body_font: str = DEFAULT_BODY_FONT
    body_font_size: int = 12
    line_height: float = 1.5
    heading_font: str = DEFAULT_BODY_FONT
    heading_color: str = DEFAULT_HEADING_COLOR
    heading_h1_color: str = ""
    heading_h2_color: str = ""
    heading_h3_color: str = ""
    heading_h4_color: str = ""
    heading_h5_color: str = ""
    heading_h6_color: str = ""
    heading_h1_size_px: int = 28
    heading_h2_size_px: int = 22
    heading_h3_size_px: int = 18
    heading_h4_size_px: int = 16
    heading_h5_size_px: int = 14
    heading_h6_size_px: int = 12
    code_font: str = DEFAULT_CODE_FONT
    code_font_size: int = 11
    code_line_height: float = 1.45
    code_theme: str = "default"
    margin_top_mm: float = 25.4
    margin_right_mm: float = 25.4
    margin_bottom_mm: float = 25.4
    margin_left_mm: float = 25.4
    footer_text: str = ""
    footer_align: str = "right"
    footer_color: str = "#6b7280"
    include_footnotes: bool = True
    footnote_font_size: float = 9.0
    footnote_text_color: str = "#374151"
    footnote_marker_color: str = "#111827"
    table_row_stripes: bool = False
    table_row_odd_color: str = "#ffffff"
    table_row_even_color: str = "#f3f4f6"
    table_cell_padding_y_px: int = 6
    table_cell_padding_x_px: int = 8
    image_scale: float = 0.9
    sanitize_html: bool = True
    show_horizontal_rules: bool = True


@dataclass(slots=True)
class CompressionOptions:
    enabled: bool = True
    profile: str = "balanced"
    remove_metadata: bool = True
    timeout_sec: int = 45
    keep_original_on_fail: bool = True


@dataclass(slots=True)
class ExportOptions:
    output_format: str = "PDF"
    page_size: str = "A4"
    compression: CompressionOptions = field(default_factory=CompressionOptions)


STYLE_OPTION_INT_BOUNDS = {
    "body_font_size": (8, 24),
    "code_font_size": (8, 24),
    "heading_h1_size_px": (8, 96),
    "heading_h2_size_px": (8, 96),
    "heading_h3_size_px": (8, 96),
    "heading_h4_size_px": (8, 96),
    "heading_h5_size_px": (8, 96),
    "heading_h6_size_px": (8, 96),
    "table_cell_padding_y_px": (0, 30),
    "table_cell_padding_x_px": (0, 40),
}
STYLE_OPTION_FLOAT_BOUNDS = {
    "line_height": (1.0, 2.4),
    "code_line_height": (1.0, 2.4),
    "margin_top_mm": (0.0, 100.0),
    "margin_right_mm": (0.0, 100.0),
    "margin_bottom_mm": (0.0, 100.0),
    "margin_left_mm": (0.0, 100.0),
    "footnote_font_size": (7.0, 16.0),
    "image_scale": (0.4, 1.0),
}
_STYLE_BOOLEAN_OPTION_NAMES = {
    "include_footnotes",
    "sanitize_html",
    "show_horizontal_rules",
    "table_row_stripes",
}


def _safe_int(value: object, fallback: int) -> int:
    try:
        return int(value)
    except (TypeError, ValueError):
        return fallback


def _safe_float(value: object, fallback: float) -> float:
    try:
        return float(value)
    except (TypeError, ValueError):
        return fallback


def sanitize_text_value(value: object) -> str:
    return (
        str(value or "").replace("\0", "").replace("\r", " ").replace("\n", " ").strip()
    )


def _normalize_footer_align(value: object, fallback: object) -> str:
    cleaned = sanitize_text_value(value).casefold()
    if cleaned in {"left", "start"}:
        return "left"
    if cleaned in {"center", "centre"}:
        return "center"
    fallback_clean = sanitize_text_value(fallback).casefold()
    if fallback_clean in {"left", "start"}:
        return "left"
    return "center" if fallback_clean == "center" else "right"


def normalize_style_option_value(
    option_name: str,
    value: object,
    fallback: object,
) -> object:
    if option_name in STYLE_OPTION_INT_BOUNDS:
        minimum, maximum = STYLE_OPTION_INT_BOUNDS[option_name]
        parsed = _safe_int(value, _safe_int(fallback, minimum))
        return max(minimum, min(maximum, parsed))

    if option_name in STYLE_OPTION_FLOAT_BOUNDS:
        minimum, maximum = STYLE_OPTION_FLOAT_BOUNDS[option_name]
        parsed = _safe_float(value, _safe_float(fallback, minimum))
        return max(minimum, min(maximum, parsed))

    if option_name == "footer_align":
        return _normalize_footer_align(value, fallback)

    if option_name in _STYLE_BOOLEAN_OPTION_NAMES:
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
    if cleaned:
        return cleaned
    if isinstance(fallback, str):
        return sanitize_text_value(fallback)
    return fallback


def style_defaults(base_style: StyleOptions | None = None) -> dict[str, object]:
    style = base_style or StyleOptions()
    return {field_.name: getattr(style, field_.name) for field_ in fields(StyleOptions)}


def style_from_option_mapping(
    values: Mapping[str, object],
    base_style: StyleOptions | None = None,
) -> StyleOptions:
    style_kwargs = style_defaults(base_style)
    for option_name, raw_value in values.items():
        if option_name not in style_kwargs:
            continue
        style_kwargs[option_name] = normalize_style_option_value(
            option_name,
            raw_value,
            style_kwargs[option_name],
        )
    return StyleOptions(**style_kwargs)
