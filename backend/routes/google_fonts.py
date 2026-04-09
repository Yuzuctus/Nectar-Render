from __future__ import annotations

import asyncio
import json
import time
from collections.abc import Iterable, Sequence
from typing import Any
from urllib.error import URLError
from urllib.request import urlopen

from fastapi import APIRouter, HTTPException, Query

router = APIRouter(prefix="/fonts", tags=["fonts"])

_GOOGLE_FONTS_METADATA_URL = "https://fonts.google.com/metadata/fonts"
_GOOGLE_FONTS_TIMEOUT_SECONDS = 8.0
_GOOGLE_FONTS_JSON_PREFIX = ")]}'"
_CACHE_TTL_SECONDS = 6 * 60 * 60
_DEFAULT_LIMIT = 50
_MAX_LIMIT = 100
_ALLOWED_CATEGORIES = {
    "all",
    "sans-serif",
    "serif",
    "display",
    "handwriting",
    "monospace",
}

_cache_lock = asyncio.Lock()
_cache_families: list[dict[str, Any]] = []
_cache_timestamp = 0.0


def _normalize_search_token(value: str) -> str:
    return "".join(char.lower() for char in value if char.isalnum())


def _normalize_font_entry(raw: dict[str, Any]) -> dict[str, Any] | None:
    family = str(raw.get("family", "")).strip()
    if not family:
        return None

    category = str(raw.get("category", "sans-serif")).strip().lower()
    if category not in _ALLOWED_CATEGORIES:
        category = "sans-serif"

    popularity_raw = raw.get("popularity", 999_999)
    try:
        popularity = int(popularity_raw)
    except (TypeError, ValueError):
        popularity = 999_999

    return {
        "family": family,
        "category": category,
        "popularity": popularity,
    }


def _parse_google_fonts_payload(payload: str) -> dict[str, Any]:
    cleaned_payload = payload.lstrip("\ufeff")
    if cleaned_payload.startswith(_GOOGLE_FONTS_JSON_PREFIX):
        cleaned_payload = cleaned_payload[len(_GOOGLE_FONTS_JSON_PREFIX) :].lstrip(
            "\r\n"
        )

    try:
        data = json.loads(cleaned_payload)
    except json.JSONDecodeError as exc:
        raise RuntimeError("Invalid Google Fonts metadata response") from exc

    if not isinstance(data, dict):
        raise RuntimeError("Google Fonts metadata has invalid format")

    return data


def _fetch_google_fonts_families() -> list[dict[str, Any]]:
    try:
        with urlopen(
            _GOOGLE_FONTS_METADATA_URL, timeout=_GOOGLE_FONTS_TIMEOUT_SECONDS
        ) as response:
            payload_bytes = response.read()
    except (URLError, TimeoutError, OSError) as exc:
        raise RuntimeError("Could not fetch Google Fonts metadata") from exc

    try:
        payload = payload_bytes.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise RuntimeError("Invalid Google Fonts metadata response") from exc

    data = _parse_google_fonts_payload(payload)

    raw_list = data.get("familyMetadataList", [])
    if not isinstance(raw_list, Sequence) or isinstance(
        raw_list, (str, bytes, bytearray)
    ):
        raise RuntimeError("Google Fonts metadata has invalid format")

    families: list[dict[str, Any]] = []
    for raw in raw_list:
        if not isinstance(raw, dict):
            continue
        normalized = _normalize_font_entry(raw)
        if normalized is not None:
            families.append(normalized)

    families.sort(
        key=lambda item: (int(item["popularity"]), str(item["family"]).lower())
    )
    return families


async def _load_families_with_cache() -> list[dict[str, Any]]:
    global _cache_families, _cache_timestamp

    now = time.monotonic()
    if _cache_families and (now - _cache_timestamp) < _CACHE_TTL_SECONDS:
        return _cache_families

    async with _cache_lock:
        now = time.monotonic()
        if _cache_families and (now - _cache_timestamp) < _CACHE_TTL_SECONDS:
            return _cache_families

        try:
            families = await asyncio.to_thread(_fetch_google_fonts_families)
        except RuntimeError:
            if _cache_families:
                return _cache_families
            raise

        _cache_families = families
        _cache_timestamp = now
        return _cache_families


def _rank_and_filter_families(
    families: Iterable[dict[str, Any]],
    *,
    query: str,
    category: str,
) -> list[dict[str, Any]]:
    filtered_by_category = [
        family
        for family in families
        if category == "all" or str(family.get("category", "")).lower() == category
    ]

    search_token = _normalize_search_token(query)
    if not search_token:
        return filtered_by_category

    starts_with_matches: list[dict[str, Any]] = []
    contains_matches: list[dict[str, Any]] = []

    for family in filtered_by_category:
        family_name = str(family.get("family", ""))
        family_token = _normalize_search_token(family_name)
        if family_token.startswith(search_token):
            starts_with_matches.append(family)
        elif search_token in family_token:
            contains_matches.append(family)

    return starts_with_matches + contains_matches


def _paginate_families(
    families: Sequence[dict[str, Any]],
    *,
    offset: int,
    limit: int,
) -> dict[str, Any]:
    total = len(families)
    start = max(0, offset)
    end = start + max(1, min(limit, _MAX_LIMIT))
    items = families[start:end]
    return {
        "items": items,
        "total": total,
        "offset": start,
        "limit": end - start,
        "has_more": end < total,
    }


@router.get("/google")
async def search_google_fonts(
    q: str = Query(default="", max_length=64),
    offset: int = Query(default=0, ge=0),
    limit: int = Query(default=_DEFAULT_LIMIT, ge=1, le=_MAX_LIMIT),
    category: str = Query(default="all", max_length=20),
) -> dict[str, Any]:
    category_value = category.strip().lower() or "all"
    if category_value not in _ALLOWED_CATEGORIES:
        raise HTTPException(
            status_code=400,
            detail=f"Invalid category '{category}'.",
        )

    try:
        all_families = await _load_families_with_cache()
    except RuntimeError as exc:
        raise HTTPException(
            status_code=502,
            detail="Unable to fetch Google Fonts list right now.",
        ) from exc

    ranked = _rank_and_filter_families(
        all_families,
        query=q,
        category=category_value,
    )
    return _paginate_families(ranked, offset=offset, limit=limit)
