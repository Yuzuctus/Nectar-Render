from __future__ import annotations

from pathlib import Path

import pytest

from nectar_render.adapters.rendering.url_fetcher import build_safe_url_fetcher


def test_safe_url_fetcher_blocks_external_http() -> None:
    fetcher = build_safe_url_fetcher(
        base_url=Path("."),
        default_fetcher=lambda url: {"url": url},
    )

    with pytest.raises(ValueError, match="External fetch blocked"):
        fetcher("https://example.com/image.png")


def test_safe_url_fetcher_allows_google_fonts_css() -> None:
    called: dict[str, str] = {}

    def fake_default_fetcher(url: str) -> dict[str, str]:
        called["url"] = url
        return {"url": url}

    fetcher = build_safe_url_fetcher(
        base_url=Path("."),
        default_fetcher=fake_default_fetcher,
    )

    url = (
        "https://fonts.googleapis.com/css2?family=Nunito:wght@400;500;700&display=swap"
    )
    result = fetcher(url)

    assert result["url"] == url
    assert called["url"] == url


def test_safe_url_fetcher_allows_google_fonts_static_assets() -> None:
    called: dict[str, str] = {}

    def fake_default_fetcher(url: str) -> dict[str, str]:
        called["url"] = url
        return {"url": url}

    fetcher = build_safe_url_fetcher(
        base_url=Path("."),
        default_fetcher=fake_default_fetcher,
    )

    url = "https://fonts.gstatic.com/s/nunito/v31/XRXV3I6Li01BKofAjsOUYevN.woff2"
    result = fetcher(url)

    assert result["url"] == url
    assert called["url"] == url


def test_safe_url_fetcher_rejects_insecure_google_fonts_http() -> None:
    fetcher = build_safe_url_fetcher(
        base_url=Path("."),
        default_fetcher=lambda url: {"url": url},
    )

    with pytest.raises(ValueError, match="External fetch blocked"):
        fetcher("http://fonts.googleapis.com/css2?family=Nunito")


def test_safe_url_fetcher_blocks_file_url_without_base_root() -> None:
    fetcher = build_safe_url_fetcher(
        base_url=None,
        default_fetcher=lambda url: {"url": url},
    )

    with pytest.raises(ValueError, match="File URL blocked without base root"):
        fetcher("file:///tmp/image.png")


def test_safe_url_fetcher_blocks_relative_escape(tmp_path: Path) -> None:
    fetcher = build_safe_url_fetcher(
        base_url=tmp_path,
        default_fetcher=lambda url: {"url": url},
    )

    with pytest.raises(ValueError, match="escapes base root"):
        fetcher("../outside.png")


def test_safe_url_fetcher_allows_relative_inside_root(tmp_path: Path) -> None:
    image = tmp_path / "assets" / "pic.png"
    image.parent.mkdir(parents=True, exist_ok=True)
    image.write_bytes(b"png")
    called: dict[str, str] = {}

    def fake_default_fetcher(url: str) -> dict[str, str]:
        called["url"] = url
        return {"url": url}

    fetcher = build_safe_url_fetcher(
        base_url=tmp_path,
        default_fetcher=fake_default_fetcher,
    )

    result = fetcher("assets/pic.png")

    assert result["url"] == image.resolve().as_uri()
    assert called["url"] == image.resolve().as_uri()
