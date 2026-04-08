from __future__ import annotations

import base64
import logging
import tempfile
from collections.abc import Callable
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import Protocol

from bs4 import BeautifulSoup

from ..adapters.rendering.pdf_export import (
    build_html_from_markdown,
    export_html,
    export_pdf,
)
from ..adapters.pdf_postprocess import PdfCompressionService
from ..core.styles import (
    ExportOptions,
    OUTPUT_FORMATS,
    StyleOptions,
)


class ImageMode(Enum):
    WITH_IMAGES = "with_images"
    ALT_ONLY = "alt_only"
    STRIP = "strip"


@dataclass(slots=True)
class ConversionRequest:
    markdown_file: Path
    output_directory: Path
    style: StyleOptions
    export: ExportOptions
    markdown_text: str | None = None
    output_bytes: bool = False
    api_mode: bool = False
    image_mode: ImageMode = ImageMode.WITH_IMAGES
    assets_root: Path | None = None
    assets: dict[str, bytes] | None = None


@dataclass(slots=True)
class ConversionResult:
    html_path: Path | None = None
    pdf_path: Path | None = None
    pdf_page_count: int | None = None
    pdf_size_before_bytes: int | None = None
    pdf_size_after_bytes: int | None = None
    pdf_compression_applied: bool = False
    pdf_compression_tool: str | None = None
    output_bytes: bytes | None = None
    html_preview: str | None = None
    missing_images: list[str] = field(default_factory=list)


class CompressionResultProtocol(Protocol):
    """Protocol for compression result objects."""

    path: Path
    original_size: int
    final_size: int
    applied: bool
    tool: str | None


# MIME type mapping for image extensions
_IMAGE_MIME_TYPES: dict[str, str] = {
    ".apng": "image/apng",
    ".avif": "image/avif",
    ".bmp": "image/bmp",
    ".gif": "image/gif",
    ".jpeg": "image/jpeg",
    ".jpg": "image/jpeg",
    ".png": "image/png",
    ".svg": "image/svg+xml",
    ".tif": "image/tiff",
    ".tiff": "image/tiff",
    ".webp": "image/webp",
}


def _get_mime_type(filename: str) -> str:
    """Get MIME type for an image file."""
    ext = Path(filename).suffix.lower()
    return _IMAGE_MIME_TYPES.get(ext, "application/octet-stream")


def _embed_images_as_base64(
    html: str,
    assets: dict[str, bytes],
    assets_root: Path | None = None,
    logger: logging.Logger | None = None,
) -> str:
    """Embed images in HTML as base64 data URIs.

    This ensures HTML files are self-contained and don't depend on
    external image files that may be in a temporary directory.

    Args:
        html: The HTML content with image tags
        assets: Dictionary mapping filename to image bytes
        assets_root: Optional root path for resolving relative image paths
        logger: Optional logger for warnings

    Returns:
        HTML with images embedded as base64 data URIs
    """
    soup = BeautifulSoup(html, "html.parser")

    # Build lookup dict with normalized filenames (lowercase)
    assets_lookup: dict[str, bytes] = {
        Path(name).name.lower(): data for name, data in assets.items()
    }

    for img in soup.find_all("img"):
        src = (img.get("src") or "").strip()
        if not src:
            continue

        # Skip already embedded data URIs
        if src.startswith("data:"):
            continue

        # Skip external URLs
        if src.lower().startswith(("http://", "https://")):
            continue

        # Try to find the image in assets
        # First, try direct lookup with normalized filename
        src_filename = Path(src.replace("\\", "/")).name.lower()
        image_data = assets_lookup.get(src_filename)

        # Also try resolving from assets_root if provided
        if image_data is None and assets_root is not None:
            resolved_path = (assets_root / src).resolve()
            if resolved_path.exists() and resolved_path.is_file():
                try:
                    image_data = resolved_path.read_bytes()
                except (OSError, IOError) as exc:
                    if logger:
                        logger.warning("Failed to read image %s: %s", src, exc)
                    continue

        if image_data is None:
            if logger:
                logger.warning("Image not found for base64 embedding: %s", src)
            continue

        # Determine MIME type
        mime_type = _get_mime_type(src_filename)

        # Encode as base64 data URI
        b64_data = base64.b64encode(image_data).decode("ascii")
        data_uri = f"data:{mime_type};base64,{b64_data}"
        img["src"] = data_uri

    return str(soup)


def _find_missing_assets(markdown_text: str, assets_root: Path) -> list[str]:
    from ..adapters.rendering.markdown_pipeline import extract_referenced_images

    referenced = extract_referenced_images(markdown_text)
    if not referenced:
        return []

    available_names: set[str] = {
        path.name.lower()
        for path in assets_root.rglob("*")
        if path.is_file() and path.suffix.lower() in _IMAGE_MIME_TYPES
    }

    missing: list[str] = []
    seen_missing: set[str] = set()
    for image_name in referenced:
        key = Path(image_name).name.lower()
        if key in available_names or key in seen_missing:
            continue
        seen_missing.add(key)
        missing.append(image_name)

    return missing


def _copy_assets_to_output(
    assets: dict[str, bytes],
    output_directory: Path,
    logger: logging.Logger | None = None,
) -> None:
    """Copy asset files to the output directory.

    Args:
        assets: Dictionary mapping filename to image bytes
        output_directory: Directory to copy assets to
        logger: Optional logger for info messages
    """
    assets_dir = output_directory / "assets"
    assets_dir.mkdir(parents=True, exist_ok=True)

    for filename, data in assets.items():
        target_path = assets_dir / Path(filename).name
        target_path.write_bytes(data)
        if logger:
            logger.debug("Copied asset: %s", target_path)


def _render_pdf_bytes(document_html: str, base_url: Path | str | None) -> bytes:
    """Render PDF bytes from HTML using WeasyPrint.

    Args:
        document_html: The HTML content to render.
        base_url: Base URL for resolving relative paths (images, etc.).

    Returns:
        The rendered PDF as bytes.

    Raises:
        WeasyPrintRuntimeError: If WeasyPrint is not available.
    """
    from ..utils.weasyprint_runtime import (
        WeasyPrintRuntimeError,
        build_runtime_help,
        prepare_weasyprint_environment,
    )

    prepare_weasyprint_environment()
    try:
        from weasyprint import HTML
    except (ImportError, OSError) as exc:
        raise WeasyPrintRuntimeError(build_runtime_help(exc)) from exc

    base_url_str = str(base_url) if base_url else None
    rendered = HTML(string=document_html, base_url=base_url_str).render()
    pdf_bytes = rendered.write_pdf()
    del rendered
    return pdf_bytes


def _finalize_file_outputs(
    *,
    result: ConversionResult,
    output_directory: Path,
    stem: str,
    fmt: str,
    markdown_text: str,
    document_html: str,
    html_content_to_write: str,
    style: StyleOptions,
    export: ExportOptions,
    base_url: Path | None,
    export_html_fn: Callable[..., Path] | None,
    export_pdf_fn: Callable[..., tuple[Path, int]],
    compress_pdf_fn: Callable[..., CompressionResultProtocol],
    logger: logging.Logger,
    use_embedded_html: bool = True,
    api_mode: bool = False,
) -> None:
    """Write HTML and/or PDF outputs to disk and populate result fields.

    This function consolidates the duplicated output finalization logic
    across different image mode branches.

    Args:
        result: The ConversionResult to populate.
        output_directory: Directory to write output files.
        stem: Base filename (without extension).
        fmt: Output format ("HTML", "PDF", or "PDF+HTML").
        markdown_text: Original markdown text.
        document_html: Rendered HTML document (for PDF rendering).
        html_content_to_write: HTML content to write to file (may have embedded images).
        style: Style options.
        export: Export options.
        base_url: Base URL for resolving relative paths.
        export_html_fn: Function to export HTML (used when not embedding images).
        export_pdf_fn: Function to export PDF.
        compress_pdf_fn: Function to compress PDF.
        logger: Logger instance.
        use_embedded_html: If True, write html_content_to_write directly.
                          If False, use export_html_fn.
    """
    output_directory.mkdir(parents=True, exist_ok=True)
    html_target = output_directory / f"{stem}.html"
    pdf_target = output_directory / f"{stem}.pdf"

    if fmt in {"HTML", "PDF+HTML"}:
        if use_embedded_html:
            html_target.write_text(html_content_to_write, encoding="utf-8")
            result.html_path = html_target
            logger.info(
                "HTML export generated (with embedded images): %s", result.html_path
            )
        elif export_html_fn is not None:
            result.html_path = export_html_fn(
                markdown_text=markdown_text,
                output_path=html_target,
                style=style,
                page_size=export.page_size,
                title=stem.replace("_", " ").replace("-", " ").title(),
                assets_root=base_url,
                document_html=document_html,
                api_mode=api_mode,
            )
            logger.info("HTML export generated: %s", result.html_path)

    if fmt in {"PDF", "PDF+HTML"}:
        pdf_path, page_count = export_pdf_fn(
            markdown_text=markdown_text,
            output_path=pdf_target,
            style=style,
            page_size=export.page_size,
            title=stem.replace("_", " ").replace("-", " ").title(),
            base_url=base_url,
            compression=export.compression,
            document_html=document_html,
            api_mode=api_mode,
        )
        compression_result = compress_pdf_fn(pdf_path, export.compression)
        if not compression_result.path.exists():
            msg = f"PDF export completed but final file is missing: {compression_result.path}"
            logger.error(msg)
            raise FileNotFoundError(msg)

        result.pdf_path = compression_result.path
        result.pdf_page_count = page_count
        result.pdf_size_before_bytes = compression_result.original_size
        result.pdf_size_after_bytes = compression_result.final_size
        result.pdf_compression_applied = compression_result.applied
        result.pdf_compression_tool = compression_result.tool
        logger.info("PDF export generated: %s", result.pdf_path)
        if compression_result.applied:
            logger.info(
                "PDF compression applied | tool=%s | size=%s -> %s bytes",
                compression_result.tool,
                compression_result.original_size,
                compression_result.final_size,
            )


def execute_conversion(
    request: ConversionRequest,
    *,
    build_html_from_markdown_fn: Callable[..., str],
    export_html_fn: Callable[..., Path],
    export_pdf_fn: Callable[..., tuple[Path, int]],
    compress_pdf_fn: Callable[..., CompressionResultProtocol],
    logger: logging.Logger,
) -> ConversionResult:
    markdown_file = request.markdown_file
    output_directory = request.output_directory
    style = request.style
    export = request.export

    # Validate output format early
    fmt = export.output_format.upper()
    if fmt not in OUTPUT_FORMATS:
        msg = f"Invalid output_format '{export.output_format}'. Must be one of: {', '.join(OUTPUT_FORMATS)}"
        logger.error(msg)
        raise ValueError(msg)

    if request.markdown_text is None:
        if not markdown_file.exists() or not markdown_file.is_file():
            msg = f"Markdown file not found: {markdown_file}"
            logger.error(msg)
            raise FileNotFoundError(msg)

        try:
            markdown_text = markdown_file.read_text(encoding="utf-8")
        except UnicodeDecodeError as exc:
            msg = f"Markdown file is not UTF-8 encoded: {markdown_file}"
            logger.exception(msg)
            raise ValueError(msg) from exc
    else:
        markdown_text = request.markdown_text

    logger.info(
        "Conversion started | source=%s | format=%s | output=%s",
        markdown_file,
        export.output_format,
        output_directory,
    )

    stem = markdown_file.stem
    title = stem.replace("_", " ").replace("-", " ").title()

    result = ConversionResult()

    # Determine the initial assets_root for HTML rendering
    # Note: For WITH_IMAGES + assets dict mode, we defer HTML rendering until
    # after the temp directory is set up (to avoid double rendering)
    assets_root_for_render: Path | None = (
        markdown_file.parent if request.markdown_text is None else request.assets_root
    )

    # Only render HTML now if we're NOT in the WITH_IMAGES + assets dict branch
    # (that branch needs to render with the temp directory as assets_root)
    should_defer_html_render = (
        request.image_mode == ImageMode.WITH_IMAGES and request.assets is not None
    )

    document_html: str = ""
    if not should_defer_html_render:
        document_html = build_html_from_markdown_fn(
            markdown_text,
            style=style,
            page_size=export.page_size,
            title=title,
            assets_root=assets_root_for_render,
            api_mode=request.api_mode,
        )

    # Handle STRIP mode: remove all images
    if request.image_mode == ImageMode.STRIP:
        soup = BeautifulSoup(document_html, "html.parser")
        for img in soup.find_all("img"):
            img.decompose()
        document_html = str(soup)

    # Handle ALT_ONLY mode: replace images with alt text
    elif request.image_mode == ImageMode.ALT_ONLY:
        soup = BeautifulSoup(document_html, "html.parser")
        for img in soup.find_all("img"):
            alt = (img.get("alt") or "").strip()
            src = (img.get("src") or "").strip()
            replacement_text = f"[{alt if alt else src}]"
            img.replace_with(soup.new_string(replacement_text))
        document_html = str(soup)

    # Handle WITH_IMAGES mode with assets_root (filesystem path)
    elif (
        request.image_mode == ImageMode.WITH_IMAGES and request.assets_root is not None
    ):
        if request.markdown_text is not None:
            missing = _find_missing_assets(markdown_text, request.assets_root)
            if missing:
                result.missing_images = missing
                logger.error("Missing assets: %s", missing)
                return result

        html_with_embedded_images = _embed_images_as_base64(
            document_html,
            request.assets or {},
            assets_root=request.assets_root,
            logger=logger,
        )

        result.html_preview = html_with_embedded_images

        if request.output_bytes:
            if fmt == "HTML":
                result.output_bytes = html_with_embedded_images.encode("utf-8")
            elif fmt in {"PDF", "PDF+HTML"}:
                result.output_bytes = _render_pdf_bytes(
                    document_html, request.assets_root
                )
            logger.info("Conversion complete | source=%s", markdown_file)
            return result

        _finalize_file_outputs(
            result=result,
            output_directory=output_directory,
            stem=stem,
            fmt=fmt,
            markdown_text=markdown_text,
            document_html=document_html,
            html_content_to_write=html_with_embedded_images,
            style=style,
            export=export,
            base_url=request.assets_root,
            export_html_fn=None,
            export_pdf_fn=export_pdf_fn,
            compress_pdf_fn=compress_pdf_fn,
            logger=logger,
            use_embedded_html=True,
            api_mode=request.api_mode,
        )
        logger.info("Conversion complete | source=%s", markdown_file)
        return result

    # Handle WITH_IMAGES mode with assets dict (uploaded bytes)
    elif request.image_mode == ImageMode.WITH_IMAGES and request.assets is not None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            for filename, data in request.assets.items():
                (tmp_path / filename).write_bytes(data)

            if request.markdown_text is not None:
                from ..adapters.rendering.markdown_pipeline import (
                    extract_referenced_images,
                )

                referenced = extract_referenced_images(markdown_text)
                missing = [fn for fn in referenced if fn not in request.assets]
                if missing:
                    result.missing_images = missing
                    logger.error("Missing assets: %s", missing)
                    return result

            document_html = build_html_from_markdown_fn(
                markdown_text,
                style=style,
                page_size=export.page_size,
                title=title,
                assets_root=tmp_path,
                api_mode=request.api_mode,
            )

            # For HTML output, embed images as base64 to avoid broken links
            # when the temporary directory is cleaned up
            html_with_embedded_images = _embed_images_as_base64(
                document_html,
                request.assets,
                assets_root=tmp_path,
                logger=logger,
            )

            result.html_preview = html_with_embedded_images

            if request.output_bytes:
                if fmt == "HTML":
                    result.output_bytes = html_with_embedded_images.encode("utf-8")
                elif fmt in {"PDF", "PDF+HTML"}:
                    result.output_bytes = _render_pdf_bytes(document_html, tmp_path)
                logger.info("Conversion complete | source=%s", markdown_file)
                return result

            _finalize_file_outputs(
                result=result,
                output_directory=output_directory,
                stem=stem,
                fmt=fmt,
                markdown_text=markdown_text,
                document_html=document_html,
                html_content_to_write=html_with_embedded_images,
                style=style,
                export=export,
                base_url=tmp_path,
                export_html_fn=None,
                export_pdf_fn=export_pdf_fn,
                compress_pdf_fn=compress_pdf_fn,
                logger=logger,
                use_embedded_html=True,
                api_mode=request.api_mode,
            )
            logger.info("Conversion complete | source=%s", markdown_file)
            return result

    # Default branch: no special image handling (STRIP/ALT_ONLY processed above,
    # or WITH_IMAGES without assets)
    result.html_preview = document_html

    if request.output_bytes:
        if fmt == "HTML":
            result.output_bytes = document_html.encode("utf-8")
        elif fmt in {"PDF", "PDF+HTML"}:
            result.output_bytes = _render_pdf_bytes(
                document_html, assets_root_for_render
            )
        logger.info("Conversion complete | source=%s", markdown_file)
        return result

    _finalize_file_outputs(
        result=result,
        output_directory=output_directory,
        stem=stem,
        fmt=fmt,
        markdown_text=markdown_text,
        document_html=document_html,
        html_content_to_write=document_html,
        style=style,
        export=export,
        base_url=markdown_file.parent,
        export_html_fn=export_html_fn,
        export_pdf_fn=export_pdf_fn,
        compress_pdf_fn=compress_pdf_fn,
        logger=logger,
        use_embedded_html=False,
        api_mode=request.api_mode,
    )
    logger.info("Conversion complete | source=%s", markdown_file)
    return result


class ConversionService:
    def __init__(self) -> None:
        self.pdf_compression = PdfCompressionService()
        self.logger = logging.getLogger(__name__)

    def convert(
        self,
        markdown_file: Path,
        output_directory: Path,
        style: StyleOptions,
        export: ExportOptions,
        *,
        markdown_text: str | None = None,
        output_bytes: bool = False,
        api_mode: bool = False,
        image_mode: ImageMode = ImageMode.WITH_IMAGES,
        assets_root: Path | None = None,
        assets: dict[str, bytes] | None = None,
    ) -> ConversionResult:
        request = ConversionRequest(
            markdown_file=markdown_file,
            output_directory=output_directory,
            style=style,
            export=export,
            markdown_text=markdown_text,
            output_bytes=output_bytes,
            api_mode=api_mode,
            image_mode=image_mode,
            assets_root=assets_root,
            assets=assets,
        )
        return execute_conversion(
            request,
            build_html_from_markdown_fn=build_html_from_markdown,
            export_html_fn=export_html,
            export_pdf_fn=export_pdf,
            compress_pdf_fn=self.pdf_compression.compress,
            logger=self.logger,
        )


def analyze_markdown(markdown_text: str) -> list[str]:
    from nectar_render.adapters.rendering.markdown_pipeline import (
        extract_referenced_images,
    )

    return extract_referenced_images(markdown_text)
