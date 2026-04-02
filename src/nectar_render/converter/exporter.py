from __future__ import annotations

from pathlib import Path
import sys

from ..config import CompressionOptions, StyleOptions
from ..utils.weasyprint_runtime import (
    WeasyPrintRuntimeError,
    build_windows_runtime_help,
    prepare_weasyprint_environment,
)
from .html_builder import build_document_html
from .markdown_parser import parse_markdown


def build_html_from_markdown(
    markdown_text: str,
    style: StyleOptions,
    page_size: str,
    title: str,
    assets_root: Path | None = None,
) -> str:
    body_html = parse_markdown(
        markdown_text,
        include_footnotes=style.include_footnotes,
        assets_root=assets_root,
        sanitize_html=style.sanitize_html,
    )
    return build_document_html(body_html=body_html, style=style, page_size=page_size, title=title)


def export_html(
    markdown_text: str,
    output_path: Path,
    style: StyleOptions,
    page_size: str,
    title: str,
    assets_root: Path | None = None,
) -> Path:
    html = build_html_from_markdown(
        markdown_text,
        style=style,
        page_size=page_size,
        title=title,
        assets_root=assets_root,
    )
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(html, encoding="utf-8")
    return output_path


def export_pdf(
    markdown_text: str,
    output_path: Path,
    style: StyleOptions,
    page_size: str,
    title: str,
    base_url: Path | None = None,
    compression: CompressionOptions | None = None,
) -> tuple[Path, int]:
    runtime_status = prepare_weasyprint_environment()
    try:
        from weasyprint import HTML
    except (ImportError, OSError) as exc:
        if sys.platform == "win32":
            raise WeasyPrintRuntimeError(build_windows_runtime_help(exc, runtime_status)) from exc
        raise

    html = build_html_from_markdown(
        markdown_text,
        style=style,
        page_size=page_size,
        title=title,
        assets_root=base_url,
    )
    output_path.parent.mkdir(parents=True, exist_ok=True)
    rendered = HTML(string=html, base_url=str(base_url) if base_url else None).render()
    page_count = len(rendered.pages)

    pdf_options: dict[str, object] = {}
    if compression and compression.enabled:
        profile = (compression.profile or "balanced").strip().lower()
        if profile == "max":
            pdf_options.update(
                {
                    "optimize_images": True,
                    "jpeg_quality": 80,
                    "dpi": 180,
                }
            )
        else:
            pdf_options.update(
                {
                    "optimize_images": True,
                    "jpeg_quality": 88,
                    "dpi": 220,
                }
            )

        if compression.remove_metadata:
            pdf_options["custom_metadata"] = False

    rendered.write_pdf(str(output_path), **pdf_options)
    return output_path, page_count
