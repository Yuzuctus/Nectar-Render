from __future__ import annotations

from pathlib import Path

from ...config import CompressionOptions, StyleOptions
from .document_renderer import (
    build_markdown_document_html,
    build_pdf_write_options,
    write_html_document,
)
from .url_fetcher import build_safe_url_fetcher
from ...utils.weasyprint_runtime import (
    WeasyPrintRuntimeError,
    build_runtime_help,
    prepare_weasyprint_environment,
)


def build_html_from_markdown(
    markdown_text: str,
    style: StyleOptions,
    page_size: str,
    title: str,
    assets_root: Path | None = None,
    api_mode: bool = False,
) -> str:
    return build_markdown_document_html(
        markdown_text,
        style=style,
        page_size=page_size,
        title=title,
        assets_root=assets_root,
        api_mode=api_mode,
    )


def export_html(
    markdown_text: str,
    output_path: Path,
    style: StyleOptions,
    page_size: str,
    title: str,
    assets_root: Path | None = None,
    document_html: str | None = None,
    api_mode: bool = False,
) -> Path:
    html = document_html
    if html is None:
        html = build_html_from_markdown(
            markdown_text,
            style=style,
            page_size=page_size,
            title=title,
            assets_root=assets_root,
            api_mode=api_mode,
        )
    return write_html_document(output_path, html)


def export_pdf(
    markdown_text: str,
    output_path: Path,
    style: StyleOptions,
    page_size: str,
    title: str,
    base_url: Path | None = None,
    compression: CompressionOptions | None = None,
    document_html: str | None = None,
    api_mode: bool = False,
) -> tuple[Path, int]:
    prepare_weasyprint_environment()
    try:
        import weasyprint
    except (ImportError, OSError) as exc:
        raise WeasyPrintRuntimeError(build_runtime_help(exc)) from exc

    HTML = weasyprint.HTML
    default_url_fetcher = getattr(weasyprint, "default_url_fetcher", None)
    if not callable(default_url_fetcher):

        def default_url_fetcher(url: str) -> dict[str, object]:
            raise ValueError(f"URL fetching unavailable for resource: {url}")

    html = document_html
    if html is None:
        html = build_html_from_markdown(
            markdown_text,
            style=style,
            page_size=page_size,
            title=title,
            assets_root=base_url,
            api_mode=api_mode,
        )
    output_path.parent.mkdir(parents=True, exist_ok=True)
    safe_url_fetcher = build_safe_url_fetcher(
        base_url=base_url,
        default_fetcher=default_url_fetcher,
    )
    rendered = HTML(
        string=html,
        base_url=str(base_url) if base_url else None,
        url_fetcher=safe_url_fetcher,
    ).render()
    page_count = len(rendered.pages)

    pdf_options = build_pdf_write_options(compression)
    rendered.write_pdf(str(output_path), **pdf_options)
    return output_path, page_count
