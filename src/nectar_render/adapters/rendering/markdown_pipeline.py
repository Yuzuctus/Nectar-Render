from __future__ import annotations

import logging
import re
import threading
from collections import OrderedDict
from pathlib import Path
from urllib.parse import unquote, urlparse

from bs4 import BeautifulSoup
from bs4.element import Tag

from .markdown_layout import apply_pagination_hints, normalize_image_blocks
from . import markdown_rendering as _markdown_rendering
from .markdown_rendering import (
    prepare_markdown,
    render_markdown_html,
    sanitize_html_fragment,
)
from nectar_render.utils.paths import is_external_or_absolute_path


logger = logging.getLogger(__name__)

_IMAGE_INDEX_CACHE: OrderedDict[str, dict[str, list[Path]]] = OrderedDict()
_IMAGE_CACHE_MAX_ROOTS = 8
_IMAGE_CACHE_LOCK = threading.Lock()
_IMAGE_EXTENSIONS = {
    ".apng",
    ".avif",
    ".bmp",
    ".gif",
    ".jpeg",
    ".jpg",
    ".png",
    ".svg",
    ".tif",
    ".tiff",
    ".webp",
}


def normalize_pagebreak_markers(markdown_text: str) -> str:
    return _markdown_rendering.normalize_pagebreak_markers(markdown_text)


def normalize_obsidian_image_embeds(markdown_text: str) -> str:
    return _markdown_rendering.normalize_obsidian_image_embeds(markdown_text)


def _is_safe_relative_path(src: str) -> bool:
    """Validate that a path is a safe relative path.

    Rejects:
    - Empty paths
    - Paths with directory traversal (../)
    - Hidden files/directories (starting with .)
    - Paths that are external or absolute
    """
    if not src or not src.strip():
        return False
    if is_external_or_absolute_path(src):
        return False

    # Normalize and decode the path
    decoded = unquote(src).strip()
    # Remove query string and fragment for path analysis
    parsed = urlparse(decoded)
    path_only = parsed.path or decoded

    # Check for directory traversal attempts
    normalized = path_only.replace("\\", "/")
    parts = normalized.split("/")
    for part in parts:
        if part == "..":
            return False
        # Optional: reject hidden files (starting with .)
        # Uncomment if needed: if part.startswith(".") and part not in {".", ""}:
        #     return False

    return True


def _normalize_local_src(src: str) -> str:
    parsed = urlparse(src)
    cleaned = parsed.path or src
    return unquote(cleaned).strip()


def _trim_image_index_cache() -> None:
    while len(_IMAGE_INDEX_CACHE) > _IMAGE_CACHE_MAX_ROOTS:
        _IMAGE_INDEX_CACHE.popitem(last=False)


def _build_image_index(
    root: Path, file_names: set[str] | None = None
) -> dict[str, list[Path]]:
    target_names = {name.lower() for name in (file_names or set())}
    index: dict[str, list[Path]] = {}
    found_names: set[str] = set()

    for path in root.rglob("*"):
        if not path.is_file():
            continue
        if path.suffix.lower() not in _IMAGE_EXTENSIONS:
            continue
        key = path.name.lower()
        if target_names and key not in target_names:
            continue
        index.setdefault(key, []).append(path)
        if target_names:
            found_names.add(key)
            if found_names == target_names:
                break
    return index


def _image_index_cache_key(root: Path) -> str:
    return str(root.resolve())


def _get_image_index(root: Path, file_names: set[str]) -> dict[str, list[Path]]:
    if not file_names:
        return {}

    cache_key = _image_index_cache_key(root)
    with _IMAGE_CACHE_LOCK:
        cached = _IMAGE_INDEX_CACHE.get(cache_key)
        if cached is None:
            cached = {}
            _IMAGE_INDEX_CACHE[cache_key] = cached
        else:
            _IMAGE_INDEX_CACHE.move_to_end(cache_key)

    missing_names = {name for name in file_names if name not in cached}
    if missing_names:
        discovered = _build_image_index(root, missing_names)
        with _IMAGE_CACHE_LOCK:
            for name in missing_names:
                cached[name] = discovered.get(name, [])
            _IMAGE_INDEX_CACHE.move_to_end(cache_key)
            _trim_image_index_cache()

    return {name: cached[name] for name in file_names if cached.get(name)}


def invalidate_image_index_cache(assets_root: Path | None = None) -> None:
    with _IMAGE_CACHE_LOCK:
        if assets_root is None:
            _IMAGE_INDEX_CACHE.clear()
            return
        _IMAGE_INDEX_CACHE.pop(_image_index_cache_key(assets_root), None)


def _pick_best_candidate(candidates: list[Path], assets_root: Path) -> Path:
    return min(
        candidates,
        key=lambda item: (len(item.relative_to(assets_root).parts), str(item).lower()),
    )


def _is_within_root(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
        return True
    except ValueError:
        return False


def _resolve_image_sources(
    html: str,
    assets_root: Path | None,
    *,
    api_mode: bool = False,
) -> str:
    """Resolve image sources to valid file paths.

    Args:
        html: The HTML content to process
        assets_root: Root directory for assets
        api_mode: If True, apply strict security rules:
            - Reject all external URLs (http://, https://)
            - Reject all absolute paths
            - Only allow relative paths to uploaded assets

    Returns:
        Modified HTML with resolved image sources
    """
    soup = BeautifulSoup(html, "html.parser")
    normalize_image_blocks(soup)
    apply_pagination_hints(soup)

    if assets_root is None:
        # In API mode without assets_root, remove all images for safety
        if api_mode:
            for img in soup.find_all("img"):
                src = (img.get("src") or "").strip()
                # Only keep data: URIs (base64 embedded images)
                if not src.startswith("data:"):
                    logger.warning(
                        "Removing image (no assets root in API mode): %s", src
                    )
                    img.decompose()
        return str(soup)

    if not assets_root.exists() or not assets_root.is_dir():
        return str(soup)

    assets_root_resolved = assets_root.resolve()

    unresolved_images: list[tuple[Tag, str]] = []

    for img in soup.find_all("img"):
        src = (img.get("src") or "").strip()

        # Skip empty sources
        if not src:
            continue

        # In API mode: strict rejection of external/absolute sources
        if api_mode:
            if is_external_or_absolute_path(src):
                # Allow data: URIs (embedded images)
                if src.startswith("data:"):
                    continue
                logger.warning(
                    "Rejecting external/absolute image source in API mode: %s", src
                )
                img.decompose()
                continue

            # Validate it's a safe relative path
            if not _is_safe_relative_path(src):
                logger.warning("Rejecting unsafe image path in API mode: %s", src)
                img.decompose()
                continue
        else:
            # CLI mode: skip external/absolute but don't remove
            if is_external_or_absolute_path(src):
                continue

        normalized_src = _normalize_local_src(src).replace("\\", "/")
        if not normalized_src:
            continue

        direct_path = (assets_root_resolved / normalized_src).resolve()
        if direct_path.exists() and direct_path.is_file():
            if not _is_within_root(direct_path, assets_root_resolved):
                logger.warning("Skipping image outside assets root: %s", normalized_src)
                img.decompose()
                continue
            try:
                rel_path = direct_path.relative_to(assets_root_resolved).as_posix()
                img["src"] = rel_path
            except ValueError:
                img["src"] = direct_path.as_uri()
            continue

        unresolved_images.append((img, normalized_src))

    if not unresolved_images:
        return str(soup)

    image_index = _get_image_index(
        assets_root_resolved,
        {Path(normalized_src).name.lower() for _, normalized_src in unresolved_images},
    )

    for img, normalized_src in unresolved_images:
        file_name = Path(normalized_src).name.lower()
        candidates = image_index.get(file_name, [])
        if not candidates:
            logger.warning("Image not found: %s", normalized_src)
            img.decompose()
            continue

        best_match = _pick_best_candidate(candidates, assets_root_resolved)
        try:
            rel_path = best_match.relative_to(assets_root_resolved).as_posix()
            img["src"] = rel_path
        except ValueError:
            img["src"] = best_match.as_uri()

    return str(soup)


def parse_markdown(
    markdown_text: str,
    include_footnotes: bool,
    assets_root: Path | None = None,
    sanitize_html: bool = True,
    *,
    api_mode: bool = False,
) -> str:
    """Parse markdown to HTML with image resolution.

    Args:
        markdown_text: Raw markdown text
        include_footnotes: Whether to include footnotes
        assets_root: Root directory for assets
        sanitize_html: Whether to sanitize HTML
        api_mode: If True, apply strict security (reject external URLs/absolute paths)

    Returns:
        Rendered HTML string
    """
    prepared = prepare_markdown(markdown_text, include_footnotes=include_footnotes)
    html = render_markdown_html(prepared)
    if sanitize_html:
        html = sanitize_html_fragment(html)
    return _resolve_image_sources(html, assets_root=assets_root, api_mode=api_mode)


_IMG_TAG_RE = re.compile(r'<img\s[^>]*src=["\']([^"\']+)["\'][^>]*>', re.IGNORECASE)
_MARKDOWN_IMAGE_RE = re.compile(
    r'!\[[^\]]*\]\(([^)\s]+)(?:\s+"[^"]*")?\)', re.IGNORECASE
)
_OBSIDIAN_IMAGE_RE = re.compile(r"!\[\[([^\]|]+)(?:\|[^\]]*)?\]\]")
_FENCED_CODE_BLOCK_RE = re.compile(r"^(`{3,}|~{3,}).*?^\1", re.MULTILINE | re.DOTALL)


def _is_position_in_code_block(markdown_text: str, position: int) -> bool:
    """Check if a position is inside a fenced code block."""
    for match in _FENCED_CODE_BLOCK_RE.finditer(markdown_text):
        if match.start() <= position < match.end():
            return True
    return False


def _is_position_in_inline_code(markdown_text: str, position: int) -> bool:
    """Check if a position is inside inline code (backticks).

    Handles single, double, and triple backtick inline code spans correctly.
    Per CommonMark spec: a code span begins with a backtick string and ends
    with a backtick string of equal length.
    """
    # Find the line containing the position
    line_start = markdown_text.rfind("\n", 0, position) + 1
    line_end = markdown_text.find("\n", position)
    if line_end == -1:
        line_end = len(markdown_text)
    line = markdown_text[line_start:line_end]
    pos_in_line = position - line_start

    # Parse inline code spans and check if position falls inside one
    i = 0
    while i < len(line):
        if line[i] == "`":
            # Count opening backticks
            open_start = i
            open_ticks = 0
            while i < len(line) and line[i] == "`":
                open_ticks += 1
                i += 1
            # Find matching closing backticks (same length)
            close_pos = -1
            j = i
            while j < len(line):
                if line[j] == "`":
                    close_start = j
                    close_ticks = 0
                    while j < len(line) and line[j] == "`":
                        close_ticks += 1
                        j += 1
                    if close_ticks == open_ticks:
                        close_pos = close_start
                        break
                else:
                    j += 1
            if close_pos != -1:
                # Check if position is inside this code span
                code_start = open_start + open_ticks
                code_end = close_pos
                if code_start <= pos_in_line < code_end:
                    return True
                i = close_pos + open_ticks
            # If no closing found, remaining line is not code (unclosed span)
        else:
            i += 1

    return False


def _normalize_image_reference(src: str) -> str:
    """Normalize an image reference for consistent comparison.

    - Decode URL-encoded characters (%20 -> space)
    - Remove query strings (?v=1.0)
    - Remove fragment identifiers (#section)
    - Extract filename only
    """
    # Decode URL encoding
    decoded = unquote(src).strip()
    # Parse URL to remove query and fragment
    parsed = urlparse(decoded)
    path_only = parsed.path or decoded
    # Normalize separators and get filename
    normalized = path_only.replace("\\", "/")
    filename = normalized.split("/")[-1]
    return filename


def extract_referenced_images(markdown_text: str) -> list[str]:
    """Extract all local image references from markdown text.

    This function:
    - Skips images inside fenced code blocks (``` or ~~~)
    - Skips images inside inline code (backticks)
    - Normalizes image references (decodes URLs, removes query/fragments)
    - Returns only filenames (not full paths)
    - Ignores external URLs (http://, https://)

    Args:
        markdown_text: Raw markdown text

    Returns:
        Sorted list of unique local image filenames referenced
    """
    found: set[str] = set()

    # Process HTML img tags
    for match in _IMG_TAG_RE.finditer(markdown_text):
        if _is_position_in_code_block(markdown_text, match.start()):
            continue
        if _is_position_in_inline_code(markdown_text, match.start()):
            continue

        src = match.group(1).strip()
        if src.lower().startswith(("http://", "https://", "data:")):
            continue

        src_clean = _normalize_image_reference(src)
        if src_clean:
            found.add(src_clean)

    # Process standard Markdown image syntax ![alt](src)
    for match in _MARKDOWN_IMAGE_RE.finditer(markdown_text):
        if _is_position_in_code_block(markdown_text, match.start()):
            continue
        if _is_position_in_inline_code(markdown_text, match.start()):
            continue

        src = match.group(1).strip()
        if src.lower().startswith(("http://", "https://", "data:")):
            continue

        src_clean = _normalize_image_reference(src)
        if src_clean:
            found.add(src_clean)

    # Process Obsidian-style image embeds ![[image.png|alt]]
    for match in _OBSIDIAN_IMAGE_RE.finditer(markdown_text):
        if _is_position_in_code_block(markdown_text, match.start()):
            continue
        if _is_position_in_inline_code(markdown_text, match.start()):
            continue

        src = match.group(1).strip()
        if src.lower().startswith(("http://", "https://", "data:")):
            continue

        src_clean = _normalize_image_reference(src)
        if src_clean:
            found.add(src_clean)

    return sorted(found)
