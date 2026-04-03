from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path


OUTPUT_FORMATS = ("PDF", "HTML", "PDF+HTML")
PAGE_SIZES = ("A4", "Letter", "Legal", "A3", "A5")
CODE_THEMES = ("default", "friendly", "monokai", "native", "xcode", "vim", "dracula")
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
    body_font: str = "Segoe UI"
    body_font_size: int = 12
    line_height: float = 1.5
    heading_font: str = "Segoe UI"
    heading_color: str = "#1f2937"
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
    code_font: str = "Consolas"
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
    sanitize_html: bool = False

    show_horizontal_rules: bool = True


@dataclass(slots=True)
class ExportOptions:
    output_format: str = "PDF"
    page_size: str = "A4"
    compression: "CompressionOptions" = field(
        default_factory=lambda: CompressionOptions()
    )


@dataclass(slots=True)
class CompressionOptions:
    enabled: bool = True
    profile: str = "balanced"
    remove_metadata: bool = True
    timeout_sec: int = 45
    keep_original_on_fail: bool = True


@dataclass(slots=True)
class AppConfig:
    markdown_file: Path | None = None
    output_directory: Path | None = None
    preview_html_path: Path | None = None
    style: StyleOptions = field(default_factory=StyleOptions)
    export: ExportOptions = field(default_factory=ExportOptions)
    ui_theme: str = "Light"
