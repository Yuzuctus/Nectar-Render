from __future__ import annotations

from backend.routes.google_fonts import (
    _normalize_font_entry,
    _paginate_families,
    _rank_and_filter_families,
)


def test_normalize_font_entry_casts_values() -> None:
    normalized = _normalize_font_entry(
        {
            "family": "Nunito",
            "category": "sans-serif",
            "popularity": "12",
        }
    )

    assert normalized == {
        "family": "Nunito",
        "category": "sans-serif",
        "popularity": 12,
    }


def test_normalize_font_entry_rejects_empty_family() -> None:
    assert _normalize_font_entry({"family": "   "}) is None


def test_rank_and_filter_prioritizes_prefix_matches() -> None:
    families = [
        {"family": "Noto Sans", "category": "sans-serif", "popularity": 10},
        {"family": "Nunito", "category": "sans-serif", "popularity": 20},
        {"family": "Open Sans", "category": "sans-serif", "popularity": 30},
        {"family": "Nunito Sans", "category": "sans-serif", "popularity": 40},
    ]

    ranked = _rank_and_filter_families(families, query="Nu-", category="all")

    assert [entry["family"] for entry in ranked] == [
        "Nunito",
        "Nunito Sans",
    ]


def test_rank_and_filter_applies_category_filter() -> None:
    families = [
        {"family": "Nunito", "category": "sans-serif", "popularity": 20},
        {"family": "Noto Sans Mono", "category": "monospace", "popularity": 25},
    ]

    ranked = _rank_and_filter_families(
        families,
        query="No",
        category="monospace",
    )

    assert [entry["family"] for entry in ranked] == ["Noto Sans Mono"]


def test_paginate_families_returns_has_more_flag() -> None:
    families = [
        {"family": "Font A", "category": "sans-serif", "popularity": 1},
        {"family": "Font B", "category": "sans-serif", "popularity": 2},
        {"family": "Font C", "category": "sans-serif", "popularity": 3},
        {"family": "Font D", "category": "sans-serif", "popularity": 4},
    ]

    page = _paginate_families(families, offset=1, limit=2)

    assert [entry["family"] for entry in page["items"]] == ["Font B", "Font C"]
    assert page["total"] == 4
    assert page["offset"] == 1
    assert page["limit"] == 2
    assert page["has_more"] is True
