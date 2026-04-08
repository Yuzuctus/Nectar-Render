"""Pydantic DTOs for conversion request parameters.

These dataclasses mirror the Form parameters in the /convert endpoint,
providing validation and a cleaner interface for building StyleOptions.
"""

from __future__ import annotations

from dataclasses import dataclass, fields


@dataclass(frozen=True, slots=True)
class StyleOptionsDTO:
    """DTO for style-related form parameters.

    All fields default to empty string, meaning "use default or preset value".
    Non-empty values override the corresponding StyleOptions field.
    """

    body_font: str = ""
    body_font_size: str = ""
    line_height: str = ""
    heading_font: str = ""
    heading_color: str = ""
    heading_h1_color: str = ""
    heading_h2_color: str = ""
    heading_h3_color: str = ""
    heading_h4_color: str = ""
    heading_h5_color: str = ""
    heading_h6_color: str = ""
    heading_h1_size_px: str = ""
    heading_h2_size_px: str = ""
    heading_h3_size_px: str = ""
    heading_h4_size_px: str = ""
    heading_h5_size_px: str = ""
    heading_h6_size_px: str = ""
    code_font: str = ""
    code_font_size: str = ""
    code_line_height: str = ""
    code_theme: str = ""
    margin_top_mm: str = ""
    margin_right_mm: str = ""
    margin_bottom_mm: str = ""
    margin_left_mm: str = ""
    footer_text: str = ""
    footer_align: str = ""
    footer_color: str = ""
    include_footnotes: str = ""
    footnote_font_size: str = ""
    footnote_text_color: str = ""
    footnote_marker_color: str = ""
    table_row_stripes: str = ""
    table_row_odd_color: str = ""
    table_row_even_color: str = ""
    table_cell_padding_y_px: str = ""
    table_cell_padding_x_px: str = ""
    image_scale: str = ""
    sanitize_html: str = ""
    show_horizontal_rules: str = ""

    def to_mapping(self) -> dict[str, str]:
        """Convert to a mapping dict, excluding empty values.

        Returns:
            Dictionary of field name -> value for non-empty fields only.
        """
        return {
            field.name: getattr(self, field.name)
            for field in fields(self)
            if getattr(self, field.name) != ""
        }


@dataclass(frozen=True, slots=True)
class CompressionOptionsDTO:
    """DTO for PDF compression form parameters."""

    compress_pdf: str = ""
    strip_metadata: str = ""
