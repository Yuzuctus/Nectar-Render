from __future__ import annotations

from fastapi.testclient import TestClient

from backend.main import app
from backend.routes.convert import _MAX_DATA_URI_CHARS
from nectar_render.application.conversion import ConversionResult


def test_convert_with_images_reports_missing_assets() -> None:
    client = TestClient(app)

    response = client.post(
        "/convert",
        data={
            "markdown_text": "![diagram](missing.png)",
            "image_mode": "WITH_IMAGES",
            "output_format": "PDF",
        },
    )

    assert response.status_code == 422
    assert response.json() == {"missing_images": ["missing.png"]}


def test_convert_rejects_external_image_reference() -> None:
    client = TestClient(app)

    response = client.post(
        "/convert",
        data={
            "markdown_text": '<img src="https://example.com/x.png">',
            "image_mode": "WITH_IMAGES",
        },
    )

    assert response.status_code == 400
    assert "External URLs and absolute paths are not allowed" in response.text


def test_convert_passes_api_mode_to_service_and_returns_bytes(monkeypatch) -> None:
    captured: dict[str, object] = {}

    def fake_convert(self, **kwargs):  # type: ignore[no-untyped-def]
        captured.update(kwargs)
        return ConversionResult(output_bytes=b"%PDF-1.4 api")

    monkeypatch.setattr(
        "backend.routes.convert.ConversionService.convert", fake_convert
    )

    client = TestClient(app)
    response = client.post(
        "/convert",
        data={
            "markdown_text": "# Hello",
            "image_mode": "STRIP",
            "output_format": "PDF",
        },
    )

    assert response.status_code == 200
    assert response.content == b"%PDF-1.4 api"
    assert response.headers["content-type"].startswith("application/pdf")
    assert captured["api_mode"] is True


def test_preview_html_returns_json_payload(monkeypatch) -> None:
    captured: dict[str, object] = {}

    def fake_convert(self, **kwargs):  # type: ignore[no-untyped-def]
        captured.update(kwargs)
        return ConversionResult(html_preview="<p>preview</p>")

    monkeypatch.setattr(
        "backend.routes.convert.ConversionService.convert", fake_convert
    )

    client = TestClient(app)
    response = client.post(
        "/preview",
        data={
            "markdown_text": "# Preview",
            "image_mode": "STRIP",
            "preview_engine": "html",
            "page_size": "A5",
        },
    )

    assert response.status_code == 200
    assert response.json() == {
        "engine": "html",
        "page_size": "A5",
        "html": "<p>preview</p>",
    }
    assert captured["api_mode"] is True
    assert captured["output_bytes"] is True


def test_preview_pdf_returns_inline_pdf(monkeypatch) -> None:
    def fake_convert(self, **kwargs):  # type: ignore[no-untyped-def]
        return ConversionResult(output_bytes=b"%PDF-1.4 preview")

    monkeypatch.setattr(
        "backend.routes.convert.ConversionService.convert", fake_convert
    )

    client = TestClient(app)
    response = client.post(
        "/preview",
        data={
            "markdown_text": "# Preview",
            "image_mode": "STRIP",
            "preview_engine": "pdf",
        },
    )

    assert response.status_code == 200
    assert response.content == b"%PDF-1.4 preview"
    assert response.headers["content-type"].startswith("application/pdf")
    assert response.headers["content-disposition"] == 'inline; filename="preview.pdf"'


def test_convert_rejects_too_large_data_uri() -> None:
    client = TestClient(app)
    payload = "A" * (_MAX_DATA_URI_CHARS + 1)
    markdown_text = f"![inline](data:image/png;base64,{payload})"

    response = client.post(
        "/convert",
        data={
            "markdown_text": markdown_text,
            "image_mode": "STRIP",
            "output_format": "PDF",
        },
    )

    assert response.status_code == 413
    assert "Embedded data URI is too large" in response.text


def test_preview_rejects_too_large_data_uri() -> None:
    client = TestClient(app)
    payload = "A" * (_MAX_DATA_URI_CHARS + 1)
    markdown_text = f"![inline](data:image/png;base64,{payload})"

    response = client.post(
        "/preview",
        data={
            "markdown_text": markdown_text,
            "image_mode": "STRIP",
            "preview_engine": "html",
        },
    )

    assert response.status_code == 413
    assert "Embedded data URI is too large" in response.text


def test_convert_accepts_svg_asset_upload(monkeypatch) -> None:
    captured: dict[str, object] = {}

    def fake_convert(self, **kwargs):  # type: ignore[no-untyped-def]
        captured.update(kwargs)
        return ConversionResult(output_bytes=b"<html>ok</html>")

    monkeypatch.setattr(
        "backend.routes.convert.ConversionService.convert", fake_convert
    )

    client = TestClient(app)
    response = client.post(
        "/convert",
        data={
            "markdown_text": "![diagram](diagram.svg)",
            "image_mode": "WITH_IMAGES",
            "output_format": "HTML",
        },
        files={
            "assets": (
                "diagram.svg",
                b"<svg xmlns='http://www.w3.org/2000/svg'></svg>",
                "image/svg+xml",
            ),
        },
    )

    assert response.status_code == 200
    assert response.content == b"<html>ok</html>"
    assert response.headers["content-type"].startswith("text/html")
    assert captured["api_mode"] is True


def test_builtin_presets_endpoint_returns_ambre() -> None:
    client = TestClient(app)
    response = client.get("/presets/builtin")

    assert response.status_code == 200
    payload = response.json()
    assert "presets" in payload
    assert "Ambre" in payload["presets"]


def test_google_fonts_endpoint_rejects_invalid_category() -> None:
    client = TestClient(app)
    response = client.get("/fonts/google", params={"category": "not-a-category"})

    assert response.status_code == 400
    assert "Invalid category" in response.text
