from __future__ import annotations

from collections import OrderedDict
import re
from pathlib import Path
from urllib.parse import unquote
from urllib.parse import urlparse

from bs4 import BeautifulSoup
from bs4.element import Tag

from .markdown_layout import apply_pagination_hints, normalize_image_blocks
from . import markdown_rendering as _markdown_rendering
from .markdown_rendering import (
    prepare_markdown,
    render_markdown_html,
    sanitize_html_fragment,
)

_IMAGE_INDEX_CACHE: OrderedDict[str, dict[str, list[Path]]] = OrderedDict()
_IMAGE_CACHE_MAX_ROOTS = 8
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


def _trim_image_index_cache() -> None:
    while len(_IMAGE_INDEX_CACHE) > _IMAGE_CACHE_MAX_ROOTS:
        _IMAGE_INDEX_CACHE.popitem(last=False)


def _build_image_index(
    root: Path, file_names: set[str] | None = None
) -> dict[str, list[Path]]:
    target_names = {name.lower() for name in (file_names or set())}
    index: dict[str, list[Path]] = {}
    resolved_at_root: set[str] = set()
    for path in root.rglob("*"):
        if not path.is_file():
            continue
        if path.suffix.lower() not in _IMAGE_EXTENSIONS:
            continue
        key = path.name.lower()
        if target_names and key not in target_names:
            continue
        index.setdefault(key, []).append(path)
        if target_names and len(path.relative_to(root).parts) == 1:
            resolved_at_root.add(key)
            if resolved_at_root == target_names:
                break
    return index


def _image_index_cache_key(root: Path) -> str:
    return str(root.resolve())


def _get_image_index(root: Path, file_names: set[str]) -> dict[str, list[Path]]:
    if not file_names:
        return {}

    cache_key = _image_index_cache_key(root)
    cached = _IMAGE_INDEX_CACHE.get(cache_key)
    if cached is None:
        cached = {}
        _IMAGE_INDEX_CACHE[cache_key] = cached
    else:
        _IMAGE_INDEX_CACHE.move_to_end(cache_key)

    missing_names = {name for name in file_names if name not in cached}
    if missing_names:
        discovered = _build_image_index(root, missing_names)
        for name in missing_names:
            cached[name] = discovered.get(name, [])
        _IMAGE_INDEX_CACHE.move_to_end(cache_key)
        _trim_image_index_cache()

    return {name: cached[name] for name in file_names if cached.get(name)}


def invalidate_image_index_cache(assets_root: Path | None = None) -> None:
    if assets_root is None:
        _IMAGE_INDEX_CACHE.clear()
        return
    _IMAGE_INDEX_CACHE.pop(_image_index_cache_key(assets_root), None)


def _pick_best_candidate(candidates: list[Path], assets_root: Path) -> Path:
    return sorted(
        candidates,
        key=lambda item: (len(item.relative_to(assets_root).parts), str(item).lower()),
    )[0]


def _resolve_image_sources(html: str, assets_root: Path | None) -> str:
    soup = BeautifulSoup(html, "html.parser")
    normalize_image_blocks(soup)
    apply_pagination_hints(soup)

    if assets_root is None:
        return str(soup)

    if not assets_root.exists() or not assets_root.is_dir():
        return str(soup)

    unresolved_images: list[tuple[Tag, str]] = []

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

        unresolved_images.append((img, normalized_src))

    if not unresolved_images:
        return str(soup)

    image_index = _get_image_index(
        assets_root,
        {Path(normalized_src).name.lower() for _, normalized_src in unresolved_images},
    )

    for img, normalized_src in unresolved_images:
        file_name = Path(normalized_src).name.lower()
        candidates = image_index.get(file_name, [])
        if not candidates:
            continue

        best_match = _pick_best_candidate(candidates, assets_root)
        img["src"] = best_match.resolve().as_uri()

    return str(soup)


def parse_markdown(
    markdown_text: str,
    include_footnotes: bool,
    assets_root: Path | None = None,
    sanitize_html: bool = True,
) -> str:
    prepared = prepare_markdown(markdown_text, include_footnotes=include_footnotes)
    html = render_markdown_html(prepared)
    if sanitize_html:
        html = sanitize_html_fragment(html)
    return _resolve_image_sources(html, assets_root=assets_root)
