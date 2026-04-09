from __future__ import annotations

import logging
import threading
import time
from collections import OrderedDict
from pathlib import Path
from urllib.parse import unquote, urlparse

from bs4 import BeautifulSoup
from bs4.element import Tag

from .markdown_layout import apply_pagination_hints, normalize_image_blocks
from .image_references import extract_image_references
from . import markdown_rendering as _markdown_rendering
from .markdown_rendering import (
    prepare_markdown,
    render_markdown_html,
    sanitize_html_fragment,
)
from nectar_render.core.image_assets import IMAGE_EXTENSIONS
from nectar_render.utils.paths import is_external_or_absolute_path


logger = logging.getLogger(__name__)

_IMAGE_INDEX_CACHE: OrderedDict[str, dict[str, list[Path]]] = OrderedDict()
_IMAGE_CACHE_MAX_ROOTS = 8
_IMAGE_CACHE_LOCK = threading.Lock()
_IMAGE_INDEX_SCAN_MAX_ENTRIES = 50_000
_IMAGE_INDEX_SCAN_TIMEOUT_SECONDS = 8.0


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
        if part.startswith(".") and part not in {".", ""}:
            return False

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
    deadline = time.monotonic() + _IMAGE_INDEX_SCAN_TIMEOUT_SECONDS
    scanned_entries = 0

    for path in root.rglob("*"):
        scanned_entries += 1
        if scanned_entries > _IMAGE_INDEX_SCAN_MAX_ENTRIES:
            logger.warning(
                "Image index scan reached max entries (%s) for root %s",
                _IMAGE_INDEX_SCAN_MAX_ENTRIES,
                root,
            )
            break
        if time.monotonic() > deadline:
            logger.warning(
                "Image index scan timed out after %.1fs for root %s",
                _IMAGE_INDEX_SCAN_TIMEOUT_SECONDS,
                root,
            )
            break
        if not path.is_file():
            continue
        if path.suffix.lower() not in IMAGE_EXTENSIONS:
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
        _IMAGE_INDEX_CACHE.move_to_end(cache_key)
        missing_names = {name for name in file_names if name not in cached}

    if missing_names:
        discovered = _build_image_index(root, missing_names)
        with _IMAGE_CACHE_LOCK:
            active_cached = _IMAGE_INDEX_CACHE.get(cache_key)
            if active_cached is None:
                active_cached = {}
                _IMAGE_INDEX_CACHE[cache_key] = active_cached
            for name in missing_names:
                active_cached[name] = discovered.get(name, [])
            _IMAGE_INDEX_CACHE.move_to_end(cache_key)
            _trim_image_index_cache()

    with _IMAGE_CACHE_LOCK:
        stable_cached = _IMAGE_INDEX_CACHE.get(cache_key, {})
        return {
            name: stable_cached[name] for name in file_names if stable_cached.get(name)
        }


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
            # CLI mode: also remove external/absolute to avoid network fetches.
            if is_external_or_absolute_path(src):
                logger.warning("Removing external/absolute image source: %s", src)
                img.decompose()
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
    extraction = extract_image_references(markdown_text)
    normalized = {
        Path(ref).name.lower().strip(): Path(ref).name.strip()
        for ref in extraction.local_references
        if Path(ref).name.strip()
    }
    return sorted(normalized.values())
