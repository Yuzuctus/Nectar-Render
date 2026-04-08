"""Tests for the HTML document builder module."""

from nectar_render.adapters.rendering.html_document import (
    _clamp_float,
    _clamp_int,
    _css_color,
    build_document_html,
)
from nectar_render.config import StyleOptions


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
        html = build_document_html(
            "<pre><code>x</code></pre>", StyleOptions(code_theme="monokai"), "A4"
        )
        assert ".codehilite" in html

    def test_includes_page_size(self) -> None:
        html = build_document_html("<p>Test</p>", StyleOptions(), "A4")
        assert "A4" in html

    def test_custom_title(self) -> None:
        html = build_document_html(
            "<p>Test</p>", StyleOptions(), "A4", title="My Document"
        )
        assert "My Document" in html


class TestCssSnapshot:
    """Snapshot-style tests ensuring key CSS properties are always present."""

    def _generate_css(self, **kwargs: object) -> str:
        style = StyleOptions(**kwargs)
        return build_document_html("<p>test</p>", style, "A4")

    def test_default_style_contains_body_font(self) -> None:
        html = self._generate_css()
        assert 'font-family: "Segoe UI", sans-serif;' in html

    def test_default_style_contains_heading_rules(self) -> None:
        html = self._generate_css()
        assert "h1, h2, h3, h4, h5, h6" in html
        assert "break-after: avoid-page" in html

    def test_default_style_contains_compact_list_rules(self) -> None:
        html = self._generate_css()
        assert ".compact-list" in html
        assert "ul, ol" in html
        assert "page-break-inside: auto;" in html

    def test_default_style_contains_page_break_class(self) -> None:
        html = self._generate_css()
        assert ".page-break" in html
        assert "break-before: page" in html

    def test_default_style_contains_footnote_rules(self) -> None:
        html = self._generate_css()
        assert ".footnote" in html
        assert "float: footnote" in html
        assert ".footnote-ref" in html

    def test_default_style_contains_code_rules(self) -> None:
        html = self._generate_css()
        assert ".codehilite" in html
        assert 'font-family: "Consolas", monospace;' in html

    def test_default_style_contains_table_rules(self) -> None:
        html = self._generate_css()
        assert "border-collapse: collapse" in html
        assert "table thead" in html
        assert "display: table-header-group" in html
        assert "table tr" in html
        assert "page-break-inside: avoid;" in html

    def test_tables_are_allowed_to_split_across_pages(self) -> None:
        html = self._generate_css()
        assert (
            "table {\n  width: 100%;\n  border-collapse: collapse;\n  break-inside: auto;"
            in html
        )

    def test_heading_before_table_uses_keep_with_next_rule(self) -> None:
        html = build_document_html(
            "<h2>Data</h2><table><tr><td>x</td></tr></table>", StyleOptions(), "A4"
        )
        assert "h1 + table, h2 + table, h3 + table, h4 + table" in html
        assert "page-break-before: avoid;" in html

    def test_custom_heading_colors_appear(self) -> None:
        html = self._generate_css(
            heading_h1_color="#ff0000", heading_h2_color="#00ff00"
        )
        assert "#ff0000" in html
        assert "#00ff00" in html

    def test_table_stripes_css_present_when_enabled(self) -> None:
        html = self._generate_css(
            table_row_stripes=True,
            table_row_odd_color="#aaa",
            table_row_even_color="#bbb",
        )
        assert "nth-child(odd)" in html
        assert "#aaa" in html
        assert "#bbb" in html

    def test_table_stripes_css_absent_when_disabled(self) -> None:
        html = self._generate_css(table_row_stripes=False)
        assert "nth-child(odd)" not in html

    def test_hr_hidden_when_disabled(self) -> None:
        html = self._generate_css(show_horizontal_rules=False)
        assert "hr { display: none; }" in html

    def test_hr_visible_by_default(self) -> None:
        html = self._generate_css()
        assert "hr { display: none; }" not in html

    def test_footer_color_applied(self) -> None:
        html = self._generate_css(footer_color="#123abc")
        assert "#123abc" in html

    def test_image_scale_in_css(self) -> None:
        html = self._generate_css(image_scale=0.7)
        assert "70.0%" in html

    def test_page_size_letter(self) -> None:
        style = StyleOptions()
        html = build_document_html("<p>test</p>", style, "Letter")
        assert "Letter" in html

    def test_lang_is_english(self) -> None:
        html = self._generate_css()
        assert 'lang="en"' in html

    def test_invalid_page_size_falls_back_to_a4(self) -> None:
        html = build_document_html(
            "<p>test</p>", StyleOptions(), "bad-size; @page { size: Legal; }"
        )
        assert "size: A4;" in html
        assert "bad-size" not in html

    def test_invalid_heading_color_does_not_inject_css(self) -> None:
        html = self._generate_css(heading_h1_color="red; body{display:none}/*")
        assert "body{display:none}" not in html
        assert "h1 { color: #1f2937; font-size: 28px; }" in html

    def test_invalid_font_family_falls_back_to_safe_default(self) -> None:
        html = self._generate_css(body_font='"; body{display:none}/*')
        assert 'font-family: "Segoe UI", sans-serif;' in html
        assert 'font-family: ""; body{display:none}/*", sans-serif;' not in html
