from __future__ import annotations

import logging
import tempfile
from collections.abc import Callable
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path

from bs4 import BeautifulSoup

from ..adapters.rendering.pdf_export import (
    build_html_from_markdown,
    export_html,
    export_pdf,
)
from ..adapters.pdf_postprocess import PdfCompressionService
from ..core.styles import ExportOptions, StyleOptions


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
    image_mode: ImageMode = ImageMode.WITH_IMAGES
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


def execute_conversion(
    request: ConversionRequest,
    *,
    build_html_from_markdown_fn: Callable[..., str],
    export_html_fn: Callable[..., Path],
    export_pdf_fn: Callable[..., tuple[Path, int]],
    compress_pdf_fn: Callable[..., object],
    logger: logging.Logger,
) -> ConversionResult:
    markdown_file = request.markdown_file
    output_directory = request.output_directory
    style = request.style
    export = request.export

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

    fmt = export.output_format.upper()
    result = ConversionResult()

    document_html = build_html_from_markdown_fn(
        markdown_text,
        style=style,
        page_size=export.page_size,
        title=title,
        assets_root=markdown_file.parent if request.markdown_text is None else None,
    )

    assets_root_for_render: Path | None = (
        markdown_file.parent if request.markdown_text is None else None
    )

    if request.image_mode == ImageMode.STRIP:
        soup = BeautifulSoup(document_html, "html.parser")
        for img in soup.find_all("img"):
            img.decompose()
        document_html = str(soup)

    elif request.image_mode == ImageMode.ALT_ONLY:
        soup = BeautifulSoup(document_html, "html.parser")
        for img in soup.find_all("img"):
            alt = (img.get("alt") or "").strip()
            src = (img.get("src") or "").strip()
            replacement_text = f"[{alt if alt else src}]"
            img.replace_with(soup.new_string(replacement_text))
        document_html = str(soup)

    elif request.image_mode == ImageMode.WITH_IMAGES and request.assets is not None:
        from pathlib import Path

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
                    raise ValueError(f"Missing required assets: {missing}")

            document_html = build_html_from_markdown_fn(
                markdown_text,
                style=style,
                page_size=export.page_size,
                title=title,
                assets_root=tmp_path,
            )

            result.html_preview = document_html

            if request.output_bytes:
                if fmt == "HTML":
                    result.output_bytes = document_html.encode("utf-8")
                elif fmt in {"PDF", "PDF+HTML"}:
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

                    rendered = HTML(
                        string=document_html,
                        base_url=str(tmp_path),
                    ).render()
                    result.output_bytes = rendered.write_pdf()
                    del rendered
                logger.info("Conversion complete | source=%s", markdown_file)
                return result

            output_directory.mkdir(parents=True, exist_ok=True)
            html_target = output_directory / f"{stem}.html"
            pdf_target = output_directory / f"{stem}.pdf"

            if fmt in {"HTML", "PDF+HTML"}:
                result.html_path = export_html_fn(
                    markdown_text=markdown_text,
                    output_path=html_target,
                    style=style,
                    page_size=export.page_size,
                    title=title,
                    assets_root=tmp_path,
                    document_html=document_html,
                )
                logger.info("HTML export generated: %s", result.html_path)

            if fmt in {"PDF", "PDF+HTML"}:
                pdf_path, page_count = export_pdf_fn(
                    markdown_text=markdown_text,
                    output_path=pdf_target,
                    style=style,
                    page_size=export.page_size,
                    title=title,
                    base_url=tmp_path,
                    compression=export.compression,
                    document_html=document_html,
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

            logger.info("Conversion complete | source=%s", markdown_file)
            return result

    result.html_preview = document_html

    if request.output_bytes:
        if fmt == "HTML":
            result.output_bytes = document_html.encode("utf-8")
        elif fmt in {"PDF", "PDF+HTML"}:
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

            rendered = HTML(
                string=document_html,
                base_url=str(assets_root_for_render)
                if assets_root_for_render
                else None,
            ).render()
            result.output_bytes = rendered.write_pdf()
            del rendered
        logger.info("Conversion complete | source=%s", markdown_file)
        return result

    output_directory.mkdir(parents=True, exist_ok=True)
    html_target = output_directory / f"{stem}.html"
    pdf_target = output_directory / f"{stem}.pdf"

    if fmt in {"HTML", "PDF+HTML"}:
        result.html_path = export_html_fn(
            markdown_text=markdown_text,
            output_path=html_target,
            style=style,
            page_size=export.page_size,
            title=title,
            assets_root=markdown_file.parent,
            document_html=document_html,
        )
        logger.info("HTML export generated: %s", result.html_path)

    if fmt in {"PDF", "PDF+HTML"}:
        pdf_path, page_count = export_pdf_fn(
            markdown_text=markdown_text,
            output_path=pdf_target,
            style=style,
            page_size=export.page_size,
            title=title,
            base_url=markdown_file.parent,
            compression=export.compression,
            document_html=document_html,
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
    ) -> ConversionResult:
        request = ConversionRequest(
            markdown_file=markdown_file,
            output_directory=output_directory,
            style=style,
            export=export,
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
