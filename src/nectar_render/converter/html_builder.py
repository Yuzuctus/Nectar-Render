from __future__ import annotations

import re
from html import escape

from ..config import StyleOptions
from .highlight import build_pygments_css


_HEX_COLOR_RE = re.compile(r"^#(?:[0-9a-fA-F]{3}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$")


def _css_color(value: str, fallback: str) -> str:
    cleaned = (value or "").strip()
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


def _clamp_float(value: object, default: float, minimum: float, maximum: float) -> float:
    try:
        parsed = float(value)  # type: ignore[arg-type]
    except (TypeError, ValueError):
        return default
    return max(minimum, min(maximum, parsed))


def _css_string_literal(value: str) -> str:
    escaped = (value or "").replace("\\", "\\\\").replace('"', '\\"')
    return f'"{escaped}"'


def _base_css(style: StyleOptions, page_size: str) -> str:
    heading_color = escape(style.heading_color or "#1f2937")
    heading_h1_color = escape(style.heading_h1_color or style.heading_color or "#1f2937")
    heading_h2_color = escape(style.heading_h2_color or style.heading_color or "#1f2937")
    heading_h3_color = escape(style.heading_h3_color or style.heading_color or "#1f2937")
    heading_h4_color = escape(style.heading_h4_color or style.heading_color or "#1f2937")
    heading_h5_color = escape(style.heading_h5_color or style.heading_color or "#1f2937")
    heading_h6_color = escape(style.heading_h6_color or style.heading_color or "#1f2937")

    h1_size = _clamp_int(style.heading_h1_size_px, 28, 8, 96)
    h2_size = _clamp_int(style.heading_h2_size_px, 22, 8, 96)
    h3_size = _clamp_int(style.heading_h3_size_px, 18, 8, 96)
    h4_size = _clamp_int(style.heading_h4_size_px, 16, 8, 96)
    h5_size = _clamp_int(style.heading_h5_size_px, 14, 8, 96)
    h6_size = _clamp_int(style.heading_h6_size_px, 12, 8, 96)

    table_pad_y = _clamp_int(style.table_cell_padding_y_px, 6, 0, 30)
    table_pad_x = _clamp_int(style.table_cell_padding_x_px, 8, 0, 40)
    image_scale = _clamp_float(style.image_scale, 0.9, 0.4, 1.0)
    code_line_height = _clamp_float(style.code_line_height, 1.45, 1.0, 2.4)
    footer_align = (style.footer_align or "right").strip().lower()
    footer_slot = "@bottom-center" if footer_align == "center" else "@bottom-right"
    footer_text = (style.footer_text or "").strip()
    page_counter_css = '"Page " counter(page) " / " counter(pages)'
    footer_content = page_counter_css
    if footer_text:
      footer_content = f"{_css_string_literal(footer_text + ' — ')} {page_counter_css}"

    table_stripes_css = ""
    if style.table_row_stripes:
        odd = _css_color(style.table_row_odd_color, "#ffffff")
        even = _css_color(style.table_row_even_color, "#f3f4f6")
        table_stripes_css = f"""
table tbody tr:nth-child(odd) {{
  background-color: {odd};
}}

table tbody tr:nth-child(even) {{
  background-color: {even};
}}
"""

    hr_css = ""
    if not style.show_horizontal_rules:
        hr_css = "\nhr { display: none; }\n"

    return f"""
@page {{
  size: {escape(page_size)};
  margin: {style.margin_top_mm}mm {style.margin_right_mm}mm {style.margin_bottom_mm}mm {style.margin_left_mm}mm;
  {footer_slot} {{
    content: {footer_content};
    color: #6b7280;
    font-size: 9px;
  }}
}}

html, body {{
  font-family: {escape(style.body_font)}, sans-serif;
  font-size: {style.body_font_size}px;
  line-height: {style.line_height};
  color: #1f2937;
}}

h1, h2, h3, h4, h5, h6 {{
  font-family: {escape(style.heading_font)}, sans-serif;
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

p, blockquote, pre, table, figure {{
  break-inside: avoid;
  page-break-inside: avoid;
}}

li {{
  break-inside: avoid;
  page-break-inside: avoid;
}}

h1 + p, h1 + ul, h1 + ol,
h2 + p, h2 + ul, h2 + ol,
h3 + p, h3 + ul, h3 + ol,
h4 + p, h4 + ul, h4 + ol {{
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

.allow-break-inside {{
  break-inside: auto !important;
  page-break-inside: auto !important;
}}

pre, code {{
  font-family: {escape(style.code_font)}, monospace;
  font-size: {style.code_font_size}px;
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
  border: 1px solid #d1d5db;
  border-radius: 8px;
  overflow-x: auto;
  overflow-y: hidden;
  break-inside: avoid;
}}

p code, li code, td code, th code, blockquote code {{
  padding: 0.12em 0.35em;
  border: 1px solid #d1d5db;
  border-radius: 4px;
}}

table {{
  width: 100%;
  border-collapse: collapse;
  break-inside: avoid;
}}

table th, table td {{
  border: 1px solid #d1d5db;
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
  color: {escape(style.footnote_text_color)};
  font-size: {style.footnote_font_size}px;
}}

.footnote-ref {{
  vertical-align: super;
  font-size: 0.75em;
  color: {escape(style.footnote_marker_color)};
}}

.footnote-ref a {{
  color: inherit;
  text-decoration: none;
}}

.footnotes-list {{
  margin-top: 1.2em;
  padding-top: 0.2em;
  color: {escape(style.footnote_text_color)};
  font-size: {style.footnote_font_size}px;
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
  color: {escape(style.footnote_marker_color)};
}}

.footnote::footnote-marker {{
  content: counter(footnote) ". ";
  font-weight: 600;
  color: {escape(style.footnote_marker_color)};
}}
"""


def build_document_html(body_html: str, style: StyleOptions, page_size: str, title: str = "Markdown Export") -> str:
    pygments_css = build_pygments_css(style.code_theme)
    styles = _base_css(style, page_size)

    return f"""<!DOCTYPE html>
<html lang=\"fr\">
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
