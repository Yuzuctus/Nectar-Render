from __future__ import annotations

from pathlib import Path

from ...config import CompressionOptions, StyleOptions
from .html_document import build_document_html
from .markdown_pipeline import parse_markdown


def build_markdown_body_html(
    markdown_text: str,
    *,
    style: StyleOptions,
    assets_root: Path | None = None,
    api_mode: bool = False,
) -> str:
    return parse_markdown(
        markdown_text,
        include_footnotes=style.include_footnotes,
        assets_root=assets_root,
        sanitize_html=style.sanitize_html,
        api_mode=api_mode,
    )


def build_markdown_document_html(
    markdown_text: str,
    *,
    style: StyleOptions,
    page_size: str,
    title: str,
    assets_root: Path | None = None,
    api_mode: bool = False,
) -> str:
    body_html = build_markdown_body_html(
        markdown_text,
        style=style,
        assets_root=assets_root,
        api_mode=api_mode,
    )
    return build_document_html(
        body_html=body_html,
        style=style,
        page_size=page_size,
        title=title,
    )


def write_html_document(output_path: Path, document_html: str) -> Path:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(document_html, encoding="utf-8")
    return output_path


def build_pdf_write_options(
    compression: CompressionOptions | None,
) -> dict[str, object]:
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

    if compression and compression.remove_metadata:
        pdf_options["custom_metadata"] = False

    return pdf_options


__all__ = [
    "build_markdown_body_html",
    "build_markdown_document_html",
    "build_pdf_write_options",
    "write_html_document",
]
