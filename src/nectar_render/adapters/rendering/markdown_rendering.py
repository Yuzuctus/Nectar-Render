from __future__ import annotations

import re

import bleach
import markdown as md

from ...utils.markdown import iter_lines_outside_fences
from .footnotes import inject_paged_footnotes


_PAGEBREAK_MARKERS = [
    r"<!--\s*pagebreak\s*-->",
    r"\\pagebreak",
    r"\[\[PAGEBREAK\]\]",
]
_OBSIDIAN_IMAGE_EMBED_RE = re.compile(r"!\[\[([^\]]+)\]\]")

_ALLOWED_HTML_TAGS = [
    "a",
    "abbr",
    "acronym",
    "b",
    "blockquote",
    "br",
    "code",
    "div",
    "em",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "hr",
    "i",
    "img",
    "li",
    "ol",
    "p",
    "pre",
    "span",
    "strong",
    "table",
    "tbody",
    "td",
    "th",
    "thead",
    "tr",
    "ul",
]
_ALLOWED_HTML_ATTRIBUTES: dict[str, list[str]] = {
    "*": ["class", "id"],
    "a": ["href", "title", "name"],
    "img": ["src", "alt", "title", "width", "height"],
    "span": ["data-footnote-id"],
    "td": ["colspan", "rowspan", "align"],
    "th": ["colspan", "rowspan", "align"],
}
_ALLOWED_HTML_PROTOCOLS = ["http", "https", "mailto"]
_MARKDOWN_EXTENSIONS = [
    "fenced_code",
    "codehilite",
    "tables",
    "toc",
    "sane_lists",
    "nl2br",
    "attr_list",
]
_MARKDOWN_EXTENSION_CONFIGS = {
    "codehilite": {
        "css_class": "codehilite",
        "use_pygments": True,
        "noclasses": False,
        "guess_lang": True,
    }
}


def normalize_pagebreak_markers(markdown_text: str) -> str:
    fence_map = iter_lines_outside_fences(markdown_text)
    original_lines = markdown_text.splitlines(keepends=True)
    normalized_lines: list[str] = []

    for index, line in enumerate(original_lines):
        in_fence = fence_map[index][1] if index < len(fence_map) else False
        if in_fence:
            normalized_lines.append(line)
            continue

        normalized_line = line
        for marker in _PAGEBREAK_MARKERS:
            normalized_line = re.sub(
                marker,
                '\n\n<div class="page-break"></div>\n\n',
                normalized_line,
                flags=re.IGNORECASE,
            )
        normalized_lines.append(normalized_line)

    return "".join(normalized_lines)


def normalize_obsidian_image_embeds(markdown_text: str) -> str:
    def _replace(match: re.Match[str]) -> str:
        raw_target = (match.group(1) or "").strip()
        if not raw_target:
            return match.group(0)

        target = raw_target
        alt_text = ""
        if "|" in raw_target:
            target, alt_text = [part.strip() for part in raw_target.split("|", 1)]

        if not target:
            return match.group(0)

        return f"![{alt_text}]({target})"

    return _OBSIDIAN_IMAGE_EMBED_RE.sub(_replace, markdown_text)


def prepare_markdown(markdown_text: str, *, include_footnotes: bool) -> str:
    prepared = normalize_obsidian_image_embeds(markdown_text)
    prepared = normalize_pagebreak_markers(prepared)
    return inject_paged_footnotes(prepared, enabled=include_footnotes)


def render_markdown_html(prepared_markdown: str) -> str:
    return md.markdown(
        prepared_markdown,
        extensions=_MARKDOWN_EXTENSIONS,
        extension_configs=_MARKDOWN_EXTENSION_CONFIGS,
        output_format="html5",
    )


def sanitize_html_fragment(html: str) -> str:
    return bleach.clean(
        html,
        tags=_ALLOWED_HTML_TAGS,
        attributes=_ALLOWED_HTML_ATTRIBUTES,
        protocols=_ALLOWED_HTML_PROTOCOLS,
        strip=True,
    )


__all__ = [
    "normalize_obsidian_image_embeds",
    "normalize_pagebreak_markers",
    "prepare_markdown",
    "render_markdown_html",
    "sanitize_html_fragment",
]
