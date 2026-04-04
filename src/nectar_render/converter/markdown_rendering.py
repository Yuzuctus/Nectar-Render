"""Compatibility alias for legacy markdown rendering imports."""

from __future__ import annotations

import sys

from ..adapters.rendering import markdown_rendering as _markdown_rendering

sys.modules[__name__] = _markdown_rendering
