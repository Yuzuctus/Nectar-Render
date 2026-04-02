from __future__ import annotations

import re
from pathlib import Path
from urllib.parse import unquote
from urllib.parse import urlparse

import bleach
import markdown as md
from bs4 import BeautifulSoup
from bs4.element import NavigableString, Tag

from .footnotes import inject_paged_footnotes


_PAGEBREAK_MARKERS = [
    r"<!--\s*pagebreak\s*-->",
    r"\\pagebreak",
    r"\[\[PAGEBREAK\]\]",
]

_FENCE_START_RE = re.compile(r"^\s*(```+|~~~+)")
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

_ALLOWED_HTML_PROTOCOLS = ["http", "https", "mailto", "data", "file"]


def _replace_pagebreak_markers_outside_fences(markdown_text: str) -> str:
    lines = markdown_text.splitlines(keepends=True)
    in_fence = False
    fence_delim = ""
    normalized_lines: list[str] = []

    for line in lines:
        stripped = line.lstrip()
        fence_match = _FENCE_START_RE.match(stripped)
        if fence_match:
            fence = fence_match.group(1)
            if not in_fence:
                in_fence = True
                fence_delim = fence[0]
            elif fence[0] == fence_delim:
                in_fence = False
                fence_delim = ""
            normalized_lines.append(line)
            continue

        if in_fence:
            normalized_lines.append(line)
            continue

        normalized_line = line
        for marker in _PAGEBREAK_MARKERS:
            normalized_line = re.sub(
                marker,
                "\n\n<div class=\"page-break\"></div>\n\n",
                normalized_line,
                flags=re.IGNORECASE,
            )
        normalized_lines.append(normalized_line)

    return "".join(normalized_lines)


def normalize_pagebreak_markers(markdown_text: str) -> str:
    return _replace_pagebreak_markers_outside_fences(markdown_text)


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


def _is_external_or_absolute_src(src: str) -> bool:
    parsed = urlparse(src)
    if parsed.scheme and parsed.scheme.lower() in {"http", "https", "data", "file"}:
        return True
    if src.startswith("/") or src.startswith("\\"):
        return True
    if re.match(r"^[a-zA-Z]:[\\/]", src):
        return True
    return False


def _normalize_local_src(src: str) -> str:
    parsed = urlparse(src)
    cleaned = parsed.path or src
    return unquote(cleaned).strip()


def _build_image_index(root: Path) -> dict[str, list[Path]]:
    index: dict[str, list[Path]] = {}
    for path in root.rglob("*"):
        if not path.is_file():
            continue
        key = path.name.lower()
        index.setdefault(key, []).append(path)
    return index


def _pick_best_candidate(candidates: list[Path], assets_root: Path) -> Path:
    return sorted(
        candidates,
        key=lambda item: (len(item.relative_to(assets_root).parts), str(item).lower()),
    )[0]


def _normalize_image_blocks(soup: BeautifulSoup) -> None:
    for paragraph in list(soup.find_all("p")):
        image_nodes: list[Tag] = []
        paragraph_is_image_only = True

        for child in paragraph.contents:
            if isinstance(child, NavigableString):
                if child.strip():
                    paragraph_is_image_only = False
                    break
                continue

            if not isinstance(child, Tag):
                paragraph_is_image_only = False
                break

            if child.name == "img":
                image_nodes.append(child)
                continue

            if child.name == "br":
                continue

            paragraph_is_image_only = False
            break

        if not paragraph_is_image_only or not image_nodes:
            continue

        if len(image_nodes) == 1:
            classes = [str(name) for name in paragraph.get("class", [])]
            if "image-block" not in classes:
                paragraph["class"] = [*classes, "image-block"]
            continue

        separated_blocks: list[Tag] = []
        for image in image_nodes:
            image.extract()
            block = soup.new_tag("p")
            block["class"] = ["image-block"]
            block.append(image)
            separated_blocks.append(block)

        first_block = separated_blocks[0]
        paragraph.replace_with(first_block)
        previous_block = first_block
        for block in separated_blocks[1:]:
            previous_block.insert_after(block)
            previous_block = block


def _resolve_image_sources(html: str, assets_root: Path | None) -> str:
    soup = BeautifulSoup(html, "html.parser")
    _normalize_image_blocks(soup)

    if assets_root is None:
        return str(soup)

    if not assets_root.exists() or not assets_root.is_dir():
        return str(soup)

    image_index: dict[str, list[Path]] | None = None

    for img in soup.find_all("img"):
        src = (img.get("src") or "").strip()
        if not src or _is_external_or_absolute_src(src):
            continue

        normalized_src = _normalize_local_src(src).replace("\\", "/")
        if not normalized_src:
            continue
        direct_path = (assets_root / normalized_src).resolve()
        if direct_path.exists() and direct_path.is_file():
            img["src"] = direct_path.as_uri()
            continue

        if image_index is None:
            image_index = _build_image_index(assets_root)

        file_name = Path(normalized_src).name.lower()
        candidates = image_index.get(file_name, [])
        if not candidates:
            continue

        best_match = _pick_best_candidate(candidates, assets_root)
        img["src"] = best_match.resolve().as_uri()

    return str(soup)


def sanitize_html_fragment(html: str) -> str:
    return bleach.clean(
        html,
        tags=_ALLOWED_HTML_TAGS,
        attributes=_ALLOWED_HTML_ATTRIBUTES,
        protocols=_ALLOWED_HTML_PROTOCOLS,
        strip=True,
    )


def parse_markdown(
    markdown_text: str,
    include_footnotes: bool,
    assets_root: Path | None = None,
    sanitize_html: bool = False,
) -> str:
    prepared = normalize_obsidian_image_embeds(markdown_text)
    prepared = normalize_pagebreak_markers(prepared)
    prepared = inject_paged_footnotes(prepared, enabled=include_footnotes)

    extensions = [
        "fenced_code",
        "codehilite",
        "tables",
        "toc",
        "sane_lists",
        "nl2br",
        "attr_list",
    ]
    extension_configs = {
        "codehilite": {
            "css_class": "codehilite",
            "use_pygments": True,
            "noclasses": False,
            "guess_lang": True,
        }
    }
    html = md.markdown(
        prepared,
        extensions=extensions,
        extension_configs=extension_configs,
        output_format="html5",
    )
    html_with_resolved_images = _resolve_image_sources(html, assets_root=assets_root)
    if sanitize_html:
        return sanitize_html_fragment(html_with_resolved_images)
    return html_with_resolved_images
