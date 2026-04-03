from __future__ import annotations

from pathlib import Path

import pytest

import nectar_render.services.conversion_service as conversion_service_module
from nectar_render.config import CompressionOptions, ExportOptions, StyleOptions
from nectar_render.services.conversion_service import ConversionService
from nectar_render.services.pdf_compression_service import PdfCompressionResult


def test_convert_raises_if_final_pdf_is_missing(monkeypatch, tmp_path: Path) -> None:
    markdown_file = tmp_path / "demo.md"
    markdown_file.write_text("# Demo", encoding="utf-8")
    output_dir = tmp_path / "output"
    pdf_path = output_dir / "demo.pdf"

    def fake_export_pdf(**kwargs):
        pdf_path.parent.mkdir(parents=True, exist_ok=True)
        pdf_path.write_bytes(b"%PDF-1.4 demo")
        return pdf_path, 2

    service = ConversionService()
    monkeypatch.setattr(conversion_service_module, "export_pdf", fake_export_pdf)

    def fake_compress(path: Path, options: CompressionOptions) -> PdfCompressionResult:
        path.unlink(missing_ok=True)
        return PdfCompressionResult(
            path=path,
            applied=False,
            original_size=128,
            final_size=128,
        )

    monkeypatch.setattr(service.pdf_compression, "compress", fake_compress)

    with pytest.raises(FileNotFoundError, match="final file is missing"):
        service.convert(
            markdown_file=markdown_file,
            output_directory=output_dir,
            style=StyleOptions(),
            export=ExportOptions(output_format="PDF"),
        )
