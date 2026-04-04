from __future__ import annotations

import re
import tkinter as tk
from collections.abc import Mapping

from ...config import (
    CompressionOptions,
    ExportOptions,
    PDF_COMPRESSION_PROFILES,
    StyleOptions,
)
from ...utils.markdown import iter_lines_outside_fences
from .state_mapping import style_from_state


def capture_tk_state(tk_vars: Mapping[str, tk.Variable]) -> dict[str, object]:
    return {key: var.get() for key, var in tk_vars.items()}


def collect_style_from_tk_state(
    tk_vars: Mapping[str, tk.Variable],
    *,
    base_style: StyleOptions | None = None,
) -> StyleOptions:
    return style_from_state(capture_tk_state(tk_vars), base_style=base_style)


def collect_export_options(
    *,
    output_format: str,
    page_size: str,
    compression_enabled: bool,
    compression_profile: str,
    remove_metadata: bool,
) -> ExportOptions:
    normalized_profile = (compression_profile.strip() or "balanced").lower()
    if normalized_profile not in PDF_COMPRESSION_PROFILES:
        normalized_profile = "balanced"

    compression = CompressionOptions(
        enabled=bool(compression_enabled),
        profile=normalized_profile,
        remove_metadata=bool(remove_metadata),
    )
    return ExportOptions(
        output_format=output_format.strip() or "PDF",
        page_size=page_size.strip() or "A4",
        compression=compression,
    )


def detect_markdown_max_heading_level_from_text(markdown_text: str) -> int:
    max_level = 0
    for line, in_fence in iter_lines_outside_fences(markdown_text):
        if in_fence:
            continue
        match = re.match(r"^\s{0,3}(#{1,6})\s+\S", line)
        if not match:
            continue
        level = len(match.group(1))
        if level > max_level:
            max_level = level

    return max_level if max_level > 0 else 3


__all__ = [
    "capture_tk_state",
    "collect_export_options",
    "collect_style_from_tk_state",
    "detect_markdown_max_heading_level_from_text",
]
