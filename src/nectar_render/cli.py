"""Command-line interface for Nectar Render."""

from __future__ import annotations

import argparse
import logging
import sys
from dataclasses import replace
from pathlib import Path

from .application.conversion import ConversionService
from .core.presets import BUILTIN_PRESET_NAMES, BUILTIN_PRESET_STYLES
from .core.styles import (
    CompressionOptions,
    ExportOptions,
    OUTPUT_FORMATS,
    PAGE_SIZES,
    StyleOptions,
)
from .interfaces.desktop.state_mapping import style_from_state
from .utils.logging import configure_logging
from .utils.weasyprint_runtime import prepare_weasyprint_environment

logger = logging.getLogger(__name__)
_PRESET_HELP = ", ".join(BUILTIN_PRESET_NAMES)


def _style_from_preset(preset_name: str) -> StyleOptions | None:
    """Build a StyleOptions from a built-in preset name (case-insensitive)."""
    for name, style in BUILTIN_PRESET_STYLES.items():
        if name.casefold() == preset_name.casefold():
            return replace(style)
    return None


def _state_dict_to_style(state: dict[str, object]) -> StyleOptions:
    """Convert a preset state dict (Tk variable names) to a StyleOptions."""
    return style_from_state(state)


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
            available = ", ".join(BUILTIN_PRESET_NAMES)
            print(
                f"Error: unknown preset '{args.preset}'. Available: {available}",
                file=sys.stderr,
            )
            return 1
        style = preset_style

    fmt = args.format.upper()
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

    if (
        result.pdf_size_before_bytes
        and result.pdf_size_after_bytes
        and (
            result.pdf_compression_applied
            or result.pdf_size_after_bytes != result.pdf_size_before_bytes
        )
    ):
        before_kb = result.pdf_size_before_bytes / 1024
        after_kb = result.pdf_size_after_bytes / 1024
        print(f"PDF size: {before_kb:.1f}KB -> {after_kb:.1f}KB")

    return 0
