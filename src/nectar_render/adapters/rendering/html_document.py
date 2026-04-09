from __future__ import annotations

import re
from html import escape
from urllib.parse import quote_plus

from ...core.styles import STYLE_OPTION_FLOAT_BOUNDS, STYLE_OPTION_INT_BOUNDS
from ...config import (
    DEFAULT_BODY_FONT,
    DEFAULT_BODY_TEXT_COLOR,
    DEFAULT_BORDER_COLOR,
    DEFAULT_CODE_FONT,
    DEFAULT_FOOTER_FONT_SIZE,
    DEFAULT_HEADING_COLOR,
    PAGE_SIZES,
    StyleOptions,
)
from .highlight import build_pygments_css


_HEX_COLOR_RE = re.compile(r"^#(?:[0-9a-fA-F]{3}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$")
_CONTROL_CHAR_RE = re.compile(r"[\x00-\x1f\x7f]")
_FONT_FAMILY_RE = re.compile(r"^[A-Za-z0-9 ._+'-]{1,80}$")
_SYSTEM_FONT_FAMILIES = {
    "Arial",
    "Calibri",
    "Cambria",
    "Consolas",
    "Courier New",
    "Georgia",
    "Helvetica",
    "Lucida Console",
    "Menlo",
    "Monaco",
    "Segoe UI",
    "Tahoma",
    "Times New Roman",
    "Trebuchet MS",
    "Verdana",
}


def _css_color(value: object, fallback: str) -> str:
    cleaned = _normalize_css_text(value)
    if not cleaned:
        return fallback
    lowered = cleaned.lower()
    if lowered in {"transparent", "inherit", "currentcolor"}:
        return lowered
    if _HEX_COLOR_RE.match(cleaned):
        return cleaned
    return fallback


def _clamp_int(value: object, default: int, minimum: int, maximum: int) -> int:
    try:
        parsed = int(value)  # type: ignore[arg-type]
    except (TypeError, ValueError):
        return default
    return max(minimum, min(maximum, parsed))


def _clamp_float(
    value: object, default: float, minimum: float, maximum: float
) -> float:
    try:
        parsed = float(value)  # type: ignore[arg-type]
    except (TypeError, ValueError):
        return default
    return max(minimum, min(maximum, parsed))


def _normalize_css_text(value: object) -> str:
    text = str(value or "")
    text = _CONTROL_CHAR_RE.sub(" ", text)
    return " ".join(text.split())


def _css_string_literal(value: str) -> str:
    """Escape a value for use inside a CSS quoted string (content: "...")."""
    text = value or ""
    text = text.replace("\\", "\\\\")
    text = text.replace('"', '\\"')
    text = text.replace("\n", "\\A ")
    text = text.replace("\r", "")
    text = text.replace("\0", "\ufffd")
    return f'"{text}"'


def _css_font_family(value: object, fallback: str) -> str:
    cleaned = _normalize_css_text(value).strip("\"'")
    if not cleaned or not _FONT_FAMILY_RE.fullmatch(cleaned):
        cleaned = fallback
    return _css_string_literal(cleaned)


def _normalized_font_name(value: object, fallback: str) -> str:
    cleaned = _normalize_css_text(value).strip("\"'")
    if not cleaned or not _FONT_FAMILY_RE.fullmatch(cleaned):
        return fallback
    return cleaned


def _google_font_imports(style: StyleOptions) -> str:
    font_candidates = (
        _normalized_font_name(style.body_font, DEFAULT_BODY_FONT),
        _normalized_font_name(style.heading_font, DEFAULT_BODY_FONT),
        _normalized_font_name(style.code_font, DEFAULT_CODE_FONT),
    )

    imports: list[str] = []
    seen: set[str] = set()
    for family in font_candidates:
        if family in _SYSTEM_FONT_FAMILIES:
            continue
        if family in seen:
            continue
        seen.add(family)
        encoded_family = quote_plus(family).replace("+", "%20")
        imports.append(
            f'@import url("https://fonts.googleapis.com/css2?family={encoded_family}:wght@400;500;700&display=swap");'
        )

    if not imports:
        return ""
    return "\n".join(imports) + "\n\n"


def _normalized_page_size(page_size: object) -> str:
    page_size_map = {size.lower(): size for size in PAGE_SIZES}
    cleaned = _normalize_css_text(page_size).lower()
    return page_size_map.get(cleaned, "A4")


def _clamped_style_int(value: object, option_name: str, default: int) -> int:
    minimum, maximum = STYLE_OPTION_INT_BOUNDS[option_name]
    return _clamp_int(value, default, minimum, maximum)


def _clamped_style_float(value: object, option_name: str, default: float) -> float:
    minimum, maximum = STYLE_OPTION_FLOAT_BOUNDS[option_name]
    return _clamp_float(value, default, minimum, maximum)


def _heading_colors(style: StyleOptions, heading_color: str) -> tuple[str, ...]:
    return (
        _css_color(style.heading_h1_color, heading_color),
        _css_color(style.heading_h2_color, heading_color),
        _css_color(style.heading_h3_color, heading_color),
        _css_color(style.heading_h4_color, heading_color),
        _css_color(style.heading_h5_color, heading_color),
        _css_color(style.heading_h6_color, heading_color),
    )


_HEADING_DEFAULTS = {
    "heading_h1_size_px": 28,
    "heading_h2_size_px": 22,
    "heading_h3_size_px": 18,
    "heading_h4_size_px": 16,
    "heading_h5_size_px": 14,
    "heading_h6_size_px": 12,
}


def _heading_sizes(style: StyleOptions) -> tuple[int, ...]:
    return tuple(
        _clamped_style_int(getattr(style, k), k, v)
        for k, v in _HEADING_DEFAULTS.items()
    )


def _footer_css(style: StyleOptions) -> tuple[str, str, str, float]:
    footer_align = (style.footer_align or "right").strip().lower()
    if footer_align == "left":
        footer_slot = "@bottom-left"
    elif footer_align == "center":
        footer_slot = "@bottom-center"
    else:
        footer_slot = "@bottom-right"
    footer_color = _css_color(style.footer_color, "#6b7280")
    footer_font_size = _clamped_style_float(
        style.footer_font_size, "footer_font_size", DEFAULT_FOOTER_FONT_SIZE
    )
    footer_text = (style.footer_text or "").strip()
    page_counter_css = '"Page " counter(page) " / " counter(pages)'
    footer_content = page_counter_css
    if footer_text:
        footer_content = (
            f"{_css_string_literal(footer_text + ' - ')} {page_counter_css}"
        )
    return footer_slot, footer_color, footer_content, footer_font_size


def _table_stripes_css(style: StyleOptions) -> str:
    if not style.table_row_stripes:
        return ""

    odd = _css_color(style.table_row_odd_color, "#ffffff")
    even = _css_color(style.table_row_even_color, "#f3f4f6")
    return f"""
table tbody tr:nth-child(odd) {{
  background-color: {odd};
}}

table tbody tr:nth-child(even) {{
  background-color: {even};
}}
"""


def _horizontal_rules_css(show_horizontal_rules: bool) -> str:
    if show_horizontal_rules:
        return ""
    return "\nhr { display: none; }\n"


def _base_css(style: StyleOptions, page_size: str) -> str:
    font_imports = _google_font_imports(style)
    normalized_page_size = _normalized_page_size(page_size)
    body_font = _css_font_family(style.body_font, DEFAULT_BODY_FONT)
    heading_font = _css_font_family(style.heading_font, DEFAULT_BODY_FONT)
    code_font = _css_font_family(style.code_font, DEFAULT_CODE_FONT)

    body_font_size = _clamped_style_int(style.body_font_size, "body_font_size", 12)
    body_line_height = _clamped_style_float(style.line_height, "line_height", 1.5)
    code_font_size = _clamped_style_int(style.code_font_size, "code_font_size", 11)
    code_line_height = _clamped_style_float(
        style.code_line_height, "code_line_height", 1.45
    )
    margin_top = _clamped_style_float(style.margin_top_mm, "margin_top_mm", 25.4)
    margin_right = _clamped_style_float(style.margin_right_mm, "margin_right_mm", 25.4)
    margin_bottom = _clamped_style_float(
        style.margin_bottom_mm, "margin_bottom_mm", 25.4
    )
    margin_left = _clamped_style_float(style.margin_left_mm, "margin_left_mm", 25.4)
    footnote_font_size = _clamped_style_float(
        style.footnote_font_size, "footnote_font_size", 9.0
    )

    heading_color = _css_color(style.heading_color, DEFAULT_HEADING_COLOR)
    (
        heading_h1_color,
        heading_h2_color,
        heading_h3_color,
        heading_h4_color,
        heading_h5_color,
        heading_h6_color,
    ) = _heading_colors(style, heading_color)
    h1_size, h2_size, h3_size, h4_size, h5_size, h6_size = _heading_sizes(style)
    table_pad_y = _clamped_style_int(
        style.table_cell_padding_y_px, "table_cell_padding_y_px", 6
    )
    table_pad_x = _clamped_style_int(
        style.table_cell_padding_x_px, "table_cell_padding_x_px", 8
    )
    image_scale = _clamped_style_float(style.image_scale, "image_scale", 0.9)
    footer_slot, footer_color, footer_content, footer_font_size = _footer_css(style)
    footnote_text_color = _css_color(style.footnote_text_color, "#374151")
    footnote_marker_color = _css_color(style.footnote_marker_color, "#111827")
    body_text_color = _css_color(style.body_text_color, DEFAULT_BODY_TEXT_COLOR)
    border_color = _css_color(style.border_color, DEFAULT_BORDER_COLOR)
    table_stripes_css = _table_stripes_css(style)
    hr_css = _horizontal_rules_css(style.show_horizontal_rules)

    return f"""
{font_imports}@page {{
  size: {normalized_page_size};
  margin: {margin_top}mm {margin_right}mm {margin_bottom}mm {margin_left}mm;
  {footer_slot} {{
    content: {footer_content};
    color: {footer_color};
    font-size: {footer_font_size}px;
  }}
}}

html, body {{
  font-family: {body_font}, sans-serif;
  font-size: {body_font_size}px;
  line-height: {body_line_height};
  color: {body_text_color};
}}

h1, h2, h3, h4, h5, h6 {{
  font-family: {heading_font}, sans-serif;
  color: {heading_color};
  break-after: avoid-page;
  page-break-after: avoid;
}}

h1 {{ color: {heading_h1_color}; font-size: {h1_size}px; }}
h2 {{ color: {heading_h2_color}; font-size: {h2_size}px; }}
h3 {{ color: {heading_h3_color}; font-size: {h3_size}px; }}
h4 {{ color: {heading_h4_color}; font-size: {h4_size}px; }}
h5 {{ color: {heading_h5_color}; font-size: {h5_size}px; }}
h6 {{ color: {heading_h6_color}; font-size: {h6_size}px; }}

p, blockquote, pre, figure {{
  break-inside: avoid;
  page-break-inside: avoid;
}}

li {{
  break-inside: avoid;
  page-break-inside: avoid;
}}

ul, ol {{
  break-inside: auto;
  page-break-inside: auto;
}}

h1 + p, h1 + ul, h1 + ol,
h2 + p, h2 + ul, h2 + ol,
h3 + p, h3 + ul, h3 + ol,
h4 + p, h4 + ul, h4 + ol {{
  break-before: avoid-page;
  page-break-before: avoid;
}}

h1 + table, h2 + table, h3 + table, h4 + table {{
  break-before: avoid-page;
  page-break-before: avoid;
}}

h1 + p.image-block,
h2 + p.image-block,
h3 + p.image-block,
h4 + p.image-block {{
  break-before: auto;
  page-break-before: auto;
}}

p + ul, p + ol {{
  break-before: avoid-page;
  page-break-before: avoid;
}}

img {{
  display: block;
  max-width: {image_scale * 100:.1f}%;
  height: auto;
  margin: 0.6em auto;
  page-break-inside: avoid;
  break-inside: avoid;
}}

p.image-block {{
  text-align: center;
}}

.keep-with-next {{
  break-after: avoid-page;
  page-break-after: avoid;
}}

.keep-with-prev {{
  break-before: avoid-page;
  page-break-before: avoid;
}}

.keep-together {{
  break-inside: avoid;
  page-break-inside: avoid;
}}

.compact-list {{
  break-inside: avoid;
  page-break-inside: avoid;
}}

.allow-break-inside {{
  break-inside: auto !important;
  page-break-inside: auto !important;
}}

pre, code {{
  font-family: {code_font}, monospace;
  font-size: {code_font_size}px;
  line-height: {code_line_height};
}}

pre {{
  margin: 0;
  white-space: pre;
  word-break: normal;
  overflow-wrap: normal;
  break-inside: avoid;
}}

.codehilite {{
  margin: 1em 0;
  padding: 10px 12px;
  border: 1px solid {border_color};
  border-radius: 8px;
  overflow-x: auto;
  overflow-y: hidden;
  break-inside: avoid;
}}

p code, li code, td code, th code, blockquote code {{
  padding: 0.12em 0.35em;
  border: 1px solid {border_color};
  border-radius: 4px;
}}

table {{
  width: 100%;
  border-collapse: collapse;
  break-inside: auto;
  page-break-inside: auto;
}}

table thead {{
  display: table-header-group;
}}

table tfoot {{
  display: table-footer-group;
}}

table tr {{
  break-inside: avoid;
  page-break-inside: avoid;
}}

table th, table td {{
  border: 1px solid {border_color};
  padding: {table_pad_y}px {table_pad_x}px;
  vertical-align: top;
}}

{table_stripes_css}{hr_css}

.page-break {{
  break-before: page;
  page-break-before: always;
  height: 0;
}}

.footnote {{
  float: footnote;
  footnote-policy: auto;
  color: {footnote_text_color};
  font-size: {footnote_font_size}px;
}}

.footnote-ref {{
  vertical-align: super;
  font-size: 0.75em;
  color: {footnote_marker_color};
}}

.footnote-ref a {{
  color: inherit;
  text-decoration: none;
}}

.footnotes-list {{
  margin-top: 1.2em;
  padding-top: 0.2em;
  color: {footnote_text_color};
  font-size: {footnote_font_size}px;
}}

.footnotes-list hr {{
  margin: 0 0 0.6em 0;
}}

.footnotes-list ol {{
  margin: 0;
  padding-left: 1.2em;
}}

.footnotes-list li {{
  margin: 0.2em 0;
}}

.footnote::footnote-call {{
  content: counter(footnote);
  vertical-align: super;
  font-size: 0.75em;
  color: {footnote_marker_color};
}}

.footnote::footnote-marker {{
  content: counter(footnote) ". ";
  font-weight: 600;
  color: {footnote_marker_color};
}}
"""


def build_document_html(
    body_html: str, style: StyleOptions, page_size: str, title: str = "Markdown Export"
) -> str:
    pygments_css = build_pygments_css(style.code_theme)
    styles = _base_css(style, page_size)

    return f"""<!DOCTYPE html>
<html lang=\"en\">
<head>
  <meta charset=\"utf-8\" />
  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />
  <title>{escape(title)}</title>
  <style>
{styles}
{pygments_css}
  </style>
</head>
<body>
{body_html}
</body>
</html>
"""
