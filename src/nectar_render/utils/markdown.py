"""Shared Markdown text utilities."""

from __future__ import annotations

import re

_FENCE_START_RE = re.compile(r"^\s*(```+|~~~+)")


def iter_lines_outside_fences(text: str) -> list[tuple[str, bool]]:
    """Return a list of (line, inside_fence) tuples for each line in *text*.

    Correctly handles fences of varying lengths (e.g. ``````) per the
    CommonMark spec: a closing fence must use the same character and be
    at least as long as the opening fence.
    """
    result: list[tuple[str, bool]] = []
    in_fence = False
    fence_char = ""
    fence_len = 0

    for line in text.splitlines():
        stripped = line.lstrip()
        match = _FENCE_START_RE.match(stripped)
        if match:
            token = match.group(1)
            char = token[0]
            length = len(token)
            if not in_fence:
                in_fence = True
                fence_char = char
                fence_len = length
                result.append((line, True))
                continue
            if char == fence_char and length >= fence_len:
                in_fence = False
                fence_char = ""
                fence_len = 0
                result.append((line, True))
                continue
        result.append((line, in_fence))

    return result
