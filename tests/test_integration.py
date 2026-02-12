"""Integration tests: convert sample.md to HTML and verify output."""

import pytest
from pathlib import Path

from md_to_pdf.config import ExportOptions, StyleOptions
from md_to_pdf.converter.exporter import build_html_from_markdown


SAMPLE_MD = Path(__file__).resolve().parent.parent / "examples" / "sample.md"


@pytest.fixture
def sample_html() -> str:
    """Convert sample.md to HTML with default settings."""
    assert SAMPLE_MD.exists(), f"Sample markdown not found at {SAMPLE_MD}"
    markdown_text = SAMPLE_MD.read_text(encoding="utf-8")
    return build_html_from_markdown(
        markdown_text=markdown_text,
        style=StyleOptions(),
        page_size="A4",
        title="Integration Test",
        assets_root=SAMPLE_MD.parent,
    )


def test_sample_md_exists() -> None:
    assert SAMPLE_MD.exists()
    content = SAMPLE_MD.read_text(encoding="utf-8")
    assert len(content) > 100


def test_html_contains_headings(sample_html: str) -> None:
    assert "<h1" in sample_html
    assert "<h2" in sample_html
    assert "<h3" in sample_html


def test_html_contains_code_blocks(sample_html: str) -> None:
    assert "codehilite" in sample_html or "<pre" in sample_html


def test_html_contains_tables(sample_html: str) -> None:
    assert "<table" in sample_html
    assert "<th" in sample_html
    assert "<td" in sample_html


def test_html_contains_footnotes(sample_html: str) -> None:
    assert "footnote" in sample_html


def test_html_contains_page_break(sample_html: str) -> None:
    assert "page-break" in sample_html


def test_html_contains_images(sample_html: str) -> None:
    assert "<img" in sample_html


def test_html_contains_blockquote(sample_html: str) -> None:
    assert "<blockquote" in sample_html


def test_html_contains_lists(sample_html: str) -> None:
    assert "<ul" in sample_html
    assert "<ol" in sample_html


def test_html_is_complete_document(sample_html: str) -> None:
    lower = sample_html.lower()
    assert "<!doctype html>" in lower
    assert "<html" in lower
    assert "</html>" in lower
    assert "<style" in lower
