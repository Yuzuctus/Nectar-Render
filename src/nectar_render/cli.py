"""Command-line interface for Nectar Render."""

from __future__ import annotations

import argparse
import logging
import sys
from pathlib import Path

from .config import (
    CompressionOptions,
    ExportOptions,
    OUTPUT_FORMATS,
    PAGE_SIZES,
    StyleOptions,
)
from .services.conversion_service import ConversionService
from .ui.presets import BUILTIN_PRESET_NAMES, BUILTIN_PRESETS
from .utils.logging import configure_logging
from .utils.weasyprint_runtime import prepare_weasyprint_environment

logger = logging.getLogger(__name__)
_PRESET_HELP = ", ".join(BUILTIN_PRESET_NAMES)


def _style_from_preset(preset_name: str) -> StyleOptions | None:
    """Build a StyleOptions from a built-in preset name (case-insensitive)."""
    for name, state in BUILTIN_PRESETS.items():
        if name.lower() == preset_name.lower():
            return _state_dict_to_style(state)
    return None


def _state_dict_to_style(state: dict[str, object]) -> StyleOptions:
    """Convert a preset state dict (Tk variable names) to a StyleOptions."""
    mapping = {
        "body_font_var": "body_font",
        "heading_font_var": "heading_font",
        "body_size_var": "body_font_size",
        "line_height_var": "line_height",
        "heading_color_var": "heading_color",
        "code_font_var": "code_font",
        "code_size_var": "code_font_size",
        "code_line_height_var": "code_line_height",
        "code_theme_var": "code_theme",
        "margin_top_var": "margin_top_mm",
        "margin_right_var": "margin_right_mm",
        "margin_bottom_var": "margin_bottom_mm",
        "margin_left_var": "margin_left_mm",
        "footer_text_var": "footer_text",
        "footer_color_var": "footer_color",
        "footer_align_var": "footer_align",
        "include_notes_var": "include_footnotes",
        "footnote_size_var": "footnote_font_size",
        "footnote_text_color_var": "footnote_text_color",
        "footnote_marker_color_var": "footnote_marker_color",
        "table_stripes_var": "table_row_stripes",
        "table_odd_color_var": "table_row_odd_color",
        "table_even_color_var": "table_row_even_color",
        "table_pad_y_var": "table_cell_padding_y_px",
        "table_pad_x_var": "table_cell_padding_x_px",
        "image_scale_var": "image_scale",
        "show_horizontal_rules_var": "show_horizontal_rules",
    }
    for level in range(1, 7):
        mapping[f"heading_h{level}_color_var"] = f"heading_h{level}_color"
        mapping[f"heading_h{level}_size_var"] = f"heading_h{level}_size_px"

    kwargs: dict[str, object] = {}
    for tk_key, style_key in mapping.items():
        if tk_key in state:
            val = state[tk_key]
            if style_key == "image_scale" and isinstance(val, (int, float)):
                val = val / 100.0 if val > 1.0 else val
            if style_key == "footer_align" and isinstance(val, str):
                val = val.lower()
            kwargs[style_key] = val

    return StyleOptions(**kwargs)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="nectar-render",
        description="Nectar Render - Markdown to PDF/HTML converter",
    )
    parser.add_argument(
        "--input",
        "-i",
        type=Path,
        help="Input Markdown file (.md)",
    )
    parser.add_argument(
        "--output",
        "-o",
        type=Path,
        help="Output directory (default: <input_dir>/output)",
    )
    parser.add_argument(
        "--format",
        "-f",
        choices=[f.lower() for f in OUTPUT_FORMATS],
        default="pdf",
        help="Output format (default: pdf)",
    )
    parser.add_argument(
        "--page-size",
        choices=[s.lower() for s in PAGE_SIZES],
        default="a4",
        help="Page size (default: a4)",
    )
    parser.add_argument(
        "--preset",
        "-p",
        help=f"Built-in style preset name ({_PRESET_HELP})",
    )
    parser.add_argument(
        "--no-compression",
        action="store_true",
        help="Disable PDF compression",
    )
    return parser


def run_cli(args: argparse.Namespace) -> int:
    """Execute CLI conversion. Returns 0 on success, 1 on error."""
    configure_logging()
    prepare_weasyprint_environment()

    input_path: Path = args.input
    if not input_path.exists() or not input_path.is_file():
        print(f"Error: file not found: {input_path}", file=sys.stderr)
        return 1
    if input_path.suffix.lower() != ".md":
        print(f"Error: expected .md file, got {input_path.suffix}", file=sys.stderr)
        return 1

    output_dir: Path = args.output or (input_path.parent / "output")

    style = StyleOptions()
    if args.preset:
        preset_style = _style_from_preset(args.preset)
        if preset_style is None:
            available = ", ".join(BUILTIN_PRESETS.keys())
            print(
                f"Error: unknown preset '{args.preset}'. Available: {available}",
                file=sys.stderr,
            )
            return 1
        style = preset_style

    fmt = args.format.upper()
    if fmt == "PDF+HTML":
        fmt = "PDF+HTML"

    page_size_map = {s.lower(): s for s in PAGE_SIZES}
    page_size = page_size_map.get(args.page_size, "A4")

    compression = CompressionOptions(enabled=not args.no_compression)
    export = ExportOptions(
        output_format=fmt, page_size=page_size, compression=compression
    )

    service = ConversionService()
    try:
        result = service.convert(
            markdown_file=input_path,
            output_directory=output_dir,
            style=style,
            export=export,
        )
    except Exception as exc:
        print(f"Error: {exc}", file=sys.stderr)
        return 1

    produced = [path for path in [result.pdf_path, result.html_path] if path]
    for path in produced:
        print(f"Generated: {path}")

    if result.pdf_page_count is not None:
        print(f"PDF pages: {result.pdf_page_count}")

    if result.pdf_size_before_bytes and result.pdf_size_after_bytes:
        before_kb = result.pdf_size_before_bytes / 1024
        after_kb = result.pdf_size_after_bytes / 1024
        print(f"PDF size: {before_kb:.1f}KB -> {after_kb:.1f}KB")

    return 0
