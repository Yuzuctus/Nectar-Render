"""Compatibility alias for rendering pipeline imports."""

from __future__ import annotations

import sys

from .rendering import markdown_layout as _markdown_layout

sys.modules[__name__] = _markdown_layout
