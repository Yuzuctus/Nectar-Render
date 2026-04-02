"""Tests for the HTML builder module."""

from nectar_render.config import StyleOptions
from nectar_render.converter.html_builder import (
    _css_color,
    _clamp_int,
    _clamp_float,
    build_document_html,
)


class TestCssColor:
    def test_valid_six_digit_hex(self) -> None:
        assert _css_color("#ff00aa", "#000") == "#ff00aa"

    def test_valid_three_digit_hex(self) -> None:
        assert _css_color("#abc", "#000") == "#abc"

    def test_transparent_keyword(self) -> None:
        assert _css_color("transparent", "#000") == "transparent"

    def test_inherit_keyword(self) -> None:
        assert _css_color("inherit", "#000") == "inherit"

    def test_currentcolor_keyword(self) -> None:
        assert _css_color("currentcolor", "#000") == "currentcolor"

    def test_invalid_string_returns_fallback(self) -> None:
        assert _css_color("not-a-color", "#fff") == "#fff"

    def test_empty_string_returns_fallback(self) -> None:
        assert _css_color("", "#123456") == "#123456"

    def test_none_returns_fallback(self) -> None:
        assert _css_color(None, "#abc") == "#abc"


class TestClampInt:
    def test_value_within_range(self) -> None:
        assert _clamp_int(10, 5, 0, 20) == 10

    def test_value_below_min(self) -> None:
        assert _clamp_int(-5, 5, 0, 20) == 0

    def test_value_above_max(self) -> None:
        assert _clamp_int(100, 5, 0, 20) == 20

    def test_non_numeric_returns_default(self) -> None:
        assert _clamp_int("abc", 5, 0, 20) == 5

    def test_none_returns_default(self) -> None:
        assert _clamp_int(None, 5, 0, 20) == 5


class TestClampFloat:
    def test_value_within_range(self) -> None:
        assert _clamp_float(1.5, 1.0, 0.5, 3.0) == 1.5

    def test_value_below_min(self) -> None:
        assert _clamp_float(0.1, 1.0, 0.5, 3.0) == 0.5

    def test_value_above_max(self) -> None:
        assert _clamp_float(5.0, 1.0, 0.5, 3.0) == 3.0

    def test_non_numeric_returns_default(self) -> None:
        assert _clamp_float("abc", 1.0, 0.5, 3.0) == 1.0


class TestBuildDocumentHtml:
    def test_produces_valid_html_structure(self) -> None:
        html = build_document_html("<p>Hello</p>", StyleOptions(), "A4")
        assert "<!DOCTYPE html>" in html or "<!doctype html>" in html.lower()
        assert "<html" in html
        assert "<head>" in html
        assert "</body>" in html
        assert "Hello" in html

    def test_includes_pygments_css(self) -> None:
        html = build_document_html("<pre><code>x</code></pre>", StyleOptions(code_theme="monokai"), "A4")
        assert ".codehilite" in html

    def test_includes_page_size(self) -> None:
        html = build_document_html("<p>Test</p>", StyleOptions(), "A4")
        assert "A4" in html

    def test_custom_title(self) -> None:
        html = build_document_html("<p>Test</p>", StyleOptions(), "A4", title="My Document")
        assert "My Document" in html
