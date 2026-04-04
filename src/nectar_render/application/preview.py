from __future__ import annotations

import re
from collections.abc import Callable
from pathlib import Path

from ..adapters.rendering.pdf_export import build_html_from_markdown
from ..core.styles import StyleOptions
from ..utils.paths import default_output_dir


_SANDBOX_MARKDOWN = (
    "# Heading Level 1\n\n"
    "## Heading Level 2\n\n"
    "---\n\n"
    "### Heading Level 3\n\n"
    "Demo paragraph with `inline code` and a footnote[^note].\n\n"
    "| Column A | Column B |\n"
    "|---|---|\n"
    "| Value 1 | Value 2 |\n"
    "| Value 3 | Value 4 |\n\n"
    '```python\ndef greet(name: str) -> str:\n    return f"Hello {name}"\n\n'
    'print(greet("world"))\n```\n\n'
    "[^note]: Auto-generated footnote example.\n"
)


def resolve_output_directory(
    markdown_path: Path | None, configured_output_dir: str
) -> Path:
    configured = configured_output_dir.strip()
    if configured:
        return Path(configured)
    if markdown_path is not None:
        return default_output_dir(markdown_path)
    return default_output_dir(None)


def detect_markdown_features(markdown_text: str) -> dict[str, bool]:
    return {
        "blockquote": bool(re.search(r"(?m)^\s{0,3}>\s+", markdown_text)),
        "unordered_list": bool(re.search(r"(?m)^\s*[-*+]\s+", markdown_text)),
        "ordered_list": bool(re.search(r"(?m)^\s*\d+\.\s+", markdown_text)),
        "table": "|" in markdown_text
        and bool(re.search(r"(?m)^\s*\|?.*\|.*$", markdown_text)),
        "code_fence": bool(re.search(r"(?m)^\s*(```|~~~)", markdown_text)),
        "hr": bool(re.search(r"(?m)^\s{0,3}([-*_])\s*(\1\s*){2,}$", markdown_text)),
        "footnote": "[^" in markdown_text,
        "image": "![" in markdown_text,
        "link": bool(re.search(r"\[[^\]]+\]\([^\)]+\)", markdown_text)),
    }


def build_generated_test_markdown(
    source_markdown: str,
    *,
    heading_level_detector: Callable[[str], int],
) -> tuple[str, int]:
    max_heading = max(1, min(6, heading_level_detector(source_markdown)))
    features = detect_markdown_features(source_markdown)
    lines: list[str] = []

    for level in range(1, max_heading + 1):
        lines.append(f"{'#' * level} Heading level {level}")
        lines.append("")

    lines.append("Demo paragraph with `inline code`.")
    if features["link"]:
        lines[-1] += " Example link: [Documentation](https://example.com)."
    lines.append("")

    if features["blockquote"]:
        lines.extend(["> Blockquote example", ""])
    if features["unordered_list"]:
        lines.extend(["- Item A", "- Item B", ""])
    if features["ordered_list"]:
        lines.extend(["1. Step 1", "2. Step 2", ""])
    if features["table"]:
        lines.extend(
            [
                "| Column A | Column B |",
                "|---|---|",
                "| Value 1 | Value 2 |",
                "",
            ]
        )
    if features["code_fence"]:
        lines.extend(
            [
                "```python",
                "def greet(name: str) -> str:",
                '    return f"Hello {name}"',
                "",
                'print(greet("world"))',
                "```",
                "",
            ]
        )
    if features["hr"]:
        lines.extend(["---", ""])
    if features["image"]:
        lines.extend(["![Example image](image.png)", ""])
    if features["footnote"]:
        lines.extend(
            [
                "Text with footnote[^n1].",
                "",
                "[^n1]: Auto-generated footnote example.",
                "",
            ]
        )

    generated = "\n".join(lines).strip() or _SANDBOX_MARKDOWN
    return generated, max_heading


def build_style_preview_document(
    *,
    markdown_path: Path,
    style: StyleOptions,
    page_size: str,
    source_markdown: str,
    heading_level_detector: Callable[[str], int],
) -> tuple[str, int]:
    markdown_text, max_heading = build_generated_test_markdown(
        source_markdown,
        heading_level_detector=heading_level_detector,
    )
    html = build_html_from_markdown(
        markdown_text=markdown_text,
        style=style,
        page_size=page_size.strip() or "A4",
        title="Style test preview",
        assets_root=markdown_path.parent,
    )
    return html, max_heading
