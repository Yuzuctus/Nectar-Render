from __future__ import annotations


def safe_int(value: object, fallback: int) -> int:
    try:
        return int(value)
    except (ValueError, TypeError):
        return fallback


def safe_float(value: object, fallback: float) -> float:
    try:
        return float(value)
    except (ValueError, TypeError):
        return fallback
