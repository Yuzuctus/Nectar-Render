"""Compatibility alias for rendering pipeline imports."""

from __future__ import annotations

import sys

from .rendering import markdown_rendering as _markdown_rendering

sys.modules[__name__] = _markdown_rendering
