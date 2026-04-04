from __future__ import annotations

from .document_renderer import (
    build_markdown_body_html,
    build_markdown_document_html,
    build_pdf_write_options,
    write_html_document,
)
from .footnotes import extract_footnote_definitions, inject_paged_footnotes
from .highlight import build_pygments_css, list_available_styles, resolve_style_name
from .html_document import build_document_html
from .markdown_layout import apply_pagination_hints, normalize_image_blocks
from .markdown_pipeline import (
    invalidate_image_index_cache,
    normalize_obsidian_image_embeds,
    normalize_pagebreak_markers,
    parse_markdown,
)
from .markdown_rendering import (
    prepare_markdown,
    render_markdown_html,
    sanitize_html_fragment,
)
from .pdf_export import build_html_from_markdown, export_html, export_pdf

__all__ = [
    "apply_pagination_hints",
    "build_document_html",
    "build_html_from_markdown",
    "build_markdown_body_html",
    "build_markdown_document_html",
    "build_pdf_write_options",
    "build_pygments_css",
    "export_html",
    "export_pdf",
    "extract_footnote_definitions",
    "invalidate_image_index_cache",
    "inject_paged_footnotes",
    "list_available_styles",
    "normalize_image_blocks",
    "normalize_obsidian_image_embeds",
    "normalize_pagebreak_markers",
    "parse_markdown",
    "prepare_markdown",
    "render_markdown_html",
    "resolve_style_name",
    "sanitize_html_fragment",
    "write_html_document",
]
