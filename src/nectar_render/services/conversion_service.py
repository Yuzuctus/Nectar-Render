"""Compatibility service facade for legacy imports.

New code should prefer ``nectar_render.application.conversion`` directly.
"""

from __future__ import annotations

import logging
from pathlib import Path

from ..application.conversion import (
    ConversionRequest,
    ConversionResult,
    execute_conversion,
)
from ..config import ExportOptions, StyleOptions
from ..adapters.rendering.pdf_export import (
    build_html_from_markdown,
    export_html,
    export_pdf,
)
from .pdf_compression_service import PdfCompressionService


logger = logging.getLogger(__name__)


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
            logger=logger,
        )
