from __future__ import annotations

import logging
from dataclasses import dataclass
from pathlib import Path

from ..config import ExportOptions, StyleOptions
from ..converter.exporter import export_html, export_pdf
from .pdf_compression_service import PdfCompressionService


logger = logging.getLogger(__name__)


@dataclass(slots=True)
class ConversionResult:
    html_path: Path | None = None
    pdf_path: Path | None = None
    pdf_page_count: int | None = None
    pdf_size_before_bytes: int | None = None
    pdf_size_after_bytes: int | None = None
    pdf_compression_applied: bool = False
    pdf_compression_tool: str | None = None


class ConversionService:
    def __init__(self) -> None:
        self.pdf_compression = PdfCompressionService()

    def convert(
        self,
        markdown_file: Path,
        output_directory: Path,
        style: StyleOptions,
        export: ExportOptions,
    ) -> ConversionResult:
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

        logger.info(
            "Conversion started | source=%s | format=%s | output=%s",
            markdown_file,
            export.output_format,
            output_directory,
        )
        output_directory.mkdir(parents=True, exist_ok=True)

        stem = markdown_file.stem
        title = stem.replace("_", " ").replace("-", " ").title()
        html_target = output_directory / f"{stem}.html"
        pdf_target = output_directory / f"{stem}.pdf"

        fmt = export.output_format.upper()
        result = ConversionResult()

        if fmt in {"HTML", "PDF+HTML"}:
            result.html_path = export_html(
                markdown_text=markdown_text,
                output_path=html_target,
                style=style,
                page_size=export.page_size,
                title=title,
                assets_root=markdown_file.parent,
            )
            logger.info("HTML export generated: %s", result.html_path)

        if fmt in {"PDF", "PDF+HTML"}:
            pdf_path, page_count = export_pdf(
                markdown_text=markdown_text,
                output_path=pdf_target,
                style=style,
                page_size=export.page_size,
                title=title,
                base_url=markdown_file.parent,
                compression=export.compression,
            )
            compression_result = self.pdf_compression.compress(
                pdf_path, export.compression
            )
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
