"""Compatibility alias for rendering pipeline imports."""

from __future__ import annotations

import sys

from .rendering import footnotes as _footnotes

sys.modules[__name__] = _footnotes
