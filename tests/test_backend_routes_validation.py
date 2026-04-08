from __future__ import annotations

from types import SimpleNamespace
from pathlib import Path

import pytest
from fastapi import HTTPException

from backend.routes.convert import (
    _collect_referenced_images,
    _validate_uploaded_assets_against_markdown,
)
import nectar_render.application.conversion as conversion_module
from nectar_render.application.conversion import ConversionService
from nectar_render.core.styles import ExportOptions, StyleOptions


def test_collect_referenced_images_supports_markdown_title() -> None:
    refs = _collect_referenced_images('![Chart](images/chart.png "figure title")')
    assert refs == ["images/chart.png"]


def test_collect_referenced_images_includes_html_img_src() -> None:
    refs = _collect_referenced_images('<img src="assets/pic.png" alt="x">')
    assert refs == ["assets/pic.png"]


def test_validate_uploaded_assets_rejects_external_html_img() -> None:
    uploads = [SimpleNamespace(filename="x.png")]

    with pytest.raises(HTTPException) as exc_info:
        _validate_uploaded_assets_against_markdown(
            '<img src="http://127.0.0.1/x.png">', uploads
        )

    assert exc_info.value.status_code == 400
    assert "External URLs and absolute paths are not allowed" in str(
        exc_info.value.detail
    )


def test_validate_uploaded_assets_allows_data_uri_img() -> None:
    missing = _validate_uploaded_assets_against_markdown(
        '<img src="data:image/png;base64,AAAA" alt="inline">',
        uploads=[],
    )
    assert missing == []


def test_conversion_service_propagates_api_mode(monkeypatch, tmp_path: Path) -> None:
    markdown_file = tmp_path / "demo.md"
    markdown_file.write_text("# Demo", encoding="utf-8")
    output_dir = tmp_path / "output"
    captured: dict[str, object] = {}

    def fake_build_html_from_markdown(markdown_text: str, **kwargs: object) -> str:
        assert markdown_text == "# Demo"
        captured["build_api_mode"] = kwargs.get("api_mode")
        return "<p>demo</p>"

    def fake_export_html(**kwargs: object) -> Path:
        captured["export_api_mode"] = kwargs.get("api_mode")
        output_path = kwargs["output_path"]
        assert isinstance(output_path, Path)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(str(kwargs.get("document_html") or ""), encoding="utf-8")
        return output_path

    monkeypatch.setattr(
        conversion_module, "build_html_from_markdown", fake_build_html_from_markdown
    )
    monkeypatch.setattr(conversion_module, "export_html", fake_export_html)

    service = ConversionService()
    result = service.convert(
        markdown_file=markdown_file,
        output_directory=output_dir,
        style=StyleOptions(),
        export=ExportOptions(output_format="HTML"),
        markdown_text="# Demo",
        api_mode=True,
    )

    assert captured["build_api_mode"] is True
    assert captured["export_api_mode"] is True
    assert result.html_path == output_dir / "demo.html"
