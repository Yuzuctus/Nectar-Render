"""Compatibility alias for legacy markdown layout imports."""

from __future__ import annotations

import sys

from ..adapters.rendering import markdown_layout as _markdown_layout

sys.modules[__name__] = _markdown_layout
