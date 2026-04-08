from __future__ import annotations

import sys
import types
from pathlib import Path

import pytest

from nectar_render.adapters.rendering import pdf_export as pdf_export_module
from nectar_render.adapters.rendering.pdf_export import export_pdf
from nectar_render.config import CompressionOptions, ExportOptions, StyleOptions
from nectar_render.application.conversion import ConversionService
from nectar_render.adapters.pdf_postprocess import PdfCompressionResult


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
    monkeypatch.setattr(pdf_export_module, "export_pdf", fake_export_pdf)

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


def test_export_pdf_disables_custom_metadata_when_requested(
    monkeypatch, tmp_path: Path
) -> None:
    captured: dict[str, object] = {}

    class FakeRendered:
        pages = [object()]

        def write_pdf(self, path: str, **kwargs: object) -> None:
            captured["path"] = path
            captured["kwargs"] = kwargs
            Path(path).write_bytes(b"%PDF-1.4 demo")

    class FakeHTML:
        def __init__(self, string: str, base_url: str | None = None) -> None:
            captured["html"] = string
            captured["base_url"] = base_url

        def render(self) -> FakeRendered:
            return FakeRendered()

    monkeypatch.setattr(
        pdf_export_module, "prepare_weasyprint_environment", lambda: None
    )
    monkeypatch.setitem(sys.modules, "weasyprint", types.SimpleNamespace(HTML=FakeHTML))

    output_path, page_count = export_pdf(
        markdown_text="# Demo",
        output_path=tmp_path / "demo.pdf",
        style=StyleOptions(),
        page_size="A4",
        title="Demo",
        compression=CompressionOptions(enabled=False, remove_metadata=True),
    )

    assert output_path.exists()
    assert page_count == 1
    assert captured["kwargs"] == {"custom_metadata": False}


def test_convert_pdf_plus_html_builds_document_html_once(
    monkeypatch, tmp_path: Path
) -> None:
    markdown_file = tmp_path / "demo.md"
    markdown_file.write_text("# Demo", encoding="utf-8")
    output_dir = tmp_path / "output"

    def fake_export_html(**kwargs: object) -> Path:
        output_path = kwargs["output_path"]
        assert isinstance(output_path, Path)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(str(kwargs.get("document_html") or ""), encoding="utf-8")
        return output_path

    def fake_export_pdf(**kwargs: object) -> tuple[Path, int]:
        output_path = kwargs["output_path"]
        assert isinstance(output_path, Path)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_bytes(b"%PDF-1.4 demo")
        return output_path, 2

    class FakeCompression:
        def compress(
            self, path: Path, options: CompressionOptions
        ) -> PdfCompressionResult:
            size = path.stat().st_size
            return PdfCompressionResult(
                path=path,
                applied=False,
                original_size=size,
                final_size=size,
            )

    service = ConversionService()
    service.pdf_compression = FakeCompression()
    monkeypatch.setattr(
        service.pdf_compression,
        "compress",
        lambda p, o: PdfCompressionResult(
            path=p, applied=False, original_size=0, final_size=0
        ),
    )
    monkeypatch.setattr(pdf_export_module, "export_html", fake_export_html)
    monkeypatch.setattr(pdf_export_module, "export_pdf", fake_export_pdf)

    result = service.convert(
        markdown_file=markdown_file,
        output_directory=output_dir,
        style=StyleOptions(),
        export=ExportOptions(output_format="PDF+HTML"),
    )

    assert result.html_path == output_dir / "demo.html"
    assert result.pdf_path == output_dir / "demo.pdf"


@pytest.mark.parametrize("output_format", ["HTML", "PDF"])
def test_convert_single_output_does_not_prebuild_document_html(
    monkeypatch, tmp_path: Path, output_format: str
) -> None:
    markdown_file = tmp_path / "demo.md"
    markdown_file.write_text("# Demo", encoding="utf-8")
    output_dir = tmp_path / "output"

    def fake_export_html(**kwargs: object) -> Path:
        output_path = kwargs["output_path"]
        assert isinstance(output_path, Path)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text("<html>demo</html>", encoding="utf-8")
        return output_path

    def fake_export_pdf(**kwargs: object) -> tuple[Path, int]:
        output_path = kwargs["output_path"]
        assert isinstance(output_path, Path)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_bytes(b"%PDF-1.4 demo")
        return output_path, 1

    class FakeCompression:
        def compress(
            self, path: Path, options: CompressionOptions
        ) -> PdfCompressionResult:
            size = path.stat().st_size
            return PdfCompressionResult(
                path=path,
                applied=False,
                original_size=size,
                final_size=size,
            )

    service = ConversionService()
    service.pdf_compression = FakeCompression()
    monkeypatch.setattr(
        service.pdf_compression,
        "compress",
        lambda p, o: PdfCompressionResult(
            path=p, applied=False, original_size=0, final_size=0
        ),
    )
    monkeypatch.setattr(pdf_export_module, "export_html", fake_export_html)
    monkeypatch.setattr(pdf_export_module, "export_pdf", fake_export_pdf)

    result = service.convert(
        markdown_file=markdown_file,
        output_directory=output_dir,
        style=StyleOptions(),
        export=ExportOptions(output_format=output_format),
    )

    if output_format == "HTML":
        assert result.html_path == output_dir / "demo.html"
        assert result.pdf_path is None
    else:
        assert result.pdf_path == output_dir / "demo.pdf"
        assert result.html_path is None
