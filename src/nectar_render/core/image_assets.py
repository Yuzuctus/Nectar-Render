from __future__ import annotations

import re
import sys
from pathlib import Path

IMAGE_EXTENSIONS: frozenset[str] = frozenset(
    {
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
)

IMAGE_MIME_TYPES: dict[str, str] = {
    ".apng": "image/apng",
    ".avif": "image/avif",
    ".bmp": "image/bmp",
    ".gif": "image/gif",
    ".jpeg": "image/jpeg",
    ".jpg": "image/jpeg",
    ".png": "image/png",
    ".svg": "image/svg+xml",
    ".tif": "image/tiff",
    ".tiff": "image/tiff",
    ".webp": "image/webp",
}

_WINDOWS_RESERVED_BASENAMES = {
    "CON",
    "PRN",
    "AUX",
    "NUL",
    "COM1",
    "COM2",
    "COM3",
    "COM4",
    "COM5",
    "COM6",
    "COM7",
    "COM8",
    "COM9",
    "LPT1",
    "LPT2",
    "LPT3",
    "LPT4",
    "LPT5",
    "LPT6",
    "LPT7",
    "LPT8",
    "LPT9",
}

_UNSAFE_SVG_PATTERN = re.compile(
    r"(<\s*script\b|\son\w+\s*=|javascript\s*:|<\s*foreignObject\b|<\s*iframe\b|<\s*object\b|<\s*embed\b)",
    re.IGNORECASE,
)


def is_supported_image_filename(filename: str) -> bool:
    return Path(filename).suffix.lower() in IMAGE_EXTENSIONS


def is_windows_reserved_filename(filename: str) -> bool:
    if sys.platform != "win32":
        return False
    base_name = Path(filename).name.split(".", 1)[0].strip().upper()
    return base_name in _WINDOWS_RESERVED_BASENAMES


def is_safe_svg_bytes(payload: bytes) -> bool:
    try:
        text = payload.decode("utf-8")
    except UnicodeDecodeError:
        return False
    return _UNSAFE_SVG_PATTERN.search(text) is None


__all__ = [
    "IMAGE_EXTENSIONS",
    "IMAGE_MIME_TYPES",
    "is_supported_image_filename",
    "is_windows_reserved_filename",
    "is_safe_svg_bytes",
]
