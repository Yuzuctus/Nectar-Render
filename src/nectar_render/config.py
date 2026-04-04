from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path

from .core.styles import (
    DEFAULT_BODY_FONT,
    DEFAULT_CODE_FONT,
    DEFAULT_HEADING_COLOR,
    FALLBACK_FONTS,
    OUTPUT_FORMATS,
    PAGE_SIZES,
    PDF_COMPRESSION_PROFILES,
    UI_THEMES,
    CompressionOptions,
    ExportOptions,
    StyleOptions,
)


@dataclass(slots=True)
class AppConfig:
    markdown_file: Path | None = None
    output_directory: Path | None = None
    # Legacy field kept for state compatibility while preview remains file-based.
    preview_html_path: Path | None = None
    style: StyleOptions = field(default_factory=StyleOptions)
    export: ExportOptions = field(default_factory=ExportOptions)
    ui_theme: str = "Light"


__all__ = [
    "AppConfig",
    "CompressionOptions",
    "DEFAULT_BODY_FONT",
    "DEFAULT_CODE_FONT",
    "DEFAULT_HEADING_COLOR",
    "ExportOptions",
    "FALLBACK_FONTS",
    "OUTPUT_FORMATS",
    "PAGE_SIZES",
    "PDF_COMPRESSION_PROFILES",
    "StyleOptions",
    "UI_THEMES",
]
