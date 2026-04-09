from __future__ import annotations

import re
from collections.abc import Sequence
from dataclasses import dataclass
from urllib.parse import unquote, urlparse

from ...utils.markdown import iter_lines_outside_fences
from ...utils.paths import is_external_or_absolute_path

_STANDARD_IMAGE_RE = re.compile(
    r"!\[[^\]]*\]\((<[^>]+>|[^)\s]+)(?:\s+(?:\"[^\"]*\"|'[^']*'|\([^)]+\)))?\)",
    re.IGNORECASE,
)
_OBSIDIAN_IMAGE_RE = re.compile(r"!\[\[([^\]|]+)(?:\|[^\]]*)?\]\]")
_HTML_IMAGE_RE = re.compile(
    r"<img\b[^>]*\bsrc\s*=\s*(?:\"([^\"]+)\"|'([^']+)'|([^\s>]+))",
    re.IGNORECASE,
)


@dataclass(slots=True)
class ImageReferenceExtraction:
    all_references: list[str]
    local_references: list[str]
    rejected_external_references: list[str]


def _is_position_in_inline_code(line: str, position: int) -> bool:
    i = 0
    while i < len(line):
        if line[i] != "`":
            i += 1
            continue

        opener_start = i
        opener_len = 0
        while i < len(line) and line[i] == "`":
            opener_len += 1
            i += 1

        j = i
        while j < len(line):
            if line[j] != "`":
                j += 1
                continue
            closer_start = j
            closer_len = 0
            while j < len(line) and line[j] == "`":
                closer_len += 1
                j += 1
            if closer_len == opener_len:
                code_start = opener_start + opener_len
                code_end = closer_start
                if code_start <= position < code_end:
                    return True
                i = j
                break
        else:
            return False

    return False


def _normalize_reference_for_comparison(ref: str) -> str:
    decoded = unquote(ref).strip()
    parsed = urlparse(decoded)
    path_only = parsed.path or decoded
    normalized = path_only.replace("\\", "/")
    return normalized.split("/")[-1].lower()


def _extract_refs_from_patterns(line: str) -> list[str]:
    refs: list[str] = []
    patterns: Sequence[tuple[re.Pattern[str], int | tuple[int, ...]]] = (
        (_STANDARD_IMAGE_RE, 1),
        (_OBSIDIAN_IMAGE_RE, 1),
        (_HTML_IMAGE_RE, (1, 2, 3)),
    )

    for pattern, groups in patterns:
        for match in pattern.finditer(line):
            start = match.start()
            if _is_position_in_inline_code(line, start):
                continue
            if isinstance(groups, tuple):
                ref = ""
                for group_idx in groups:
                    candidate = (match.group(group_idx) or "").strip()
                    if candidate:
                        ref = candidate
                        break
            else:
                ref = (match.group(groups) or "").strip()

            if not ref:
                continue
            if ref.startswith("<") and ref.endswith(">") and len(ref) > 2:
                ref = ref[1:-1].strip()
            if not ref:
                continue
            refs.append(ref)

    return refs


def extract_image_references(markdown_text: str) -> ImageReferenceExtraction:
    all_refs: list[str] = []
    local_refs: list[str] = []
    rejected_refs: list[str] = []
    seen_local: set[str] = set()

    line_states = iter_lines_outside_fences(markdown_text)
    for line, in_fence in line_states:
        if in_fence:
            continue
        refs = _extract_refs_from_patterns(line)
        if not refs:
            continue

        for ref in refs:
            all_refs.append(ref)
            if ref.lower().startswith("data:"):
                continue
            if is_external_or_absolute_path(ref):
                rejected_refs.append(ref)
                continue
            normalized = _normalize_reference_for_comparison(ref)
            if not normalized or normalized in seen_local:
                continue
            seen_local.add(normalized)
            local_refs.append(ref)

    return ImageReferenceExtraction(
        all_references=all_refs,
        local_references=local_refs,
        rejected_external_references=rejected_refs,
    )


def normalize_reference_filename(ref: str) -> str:
    return _normalize_reference_for_comparison(ref)


__all__ = [
    "ImageReferenceExtraction",
    "extract_image_references",
    "normalize_reference_filename",
]
