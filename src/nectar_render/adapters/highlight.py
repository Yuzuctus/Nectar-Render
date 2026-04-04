"""Compatibility alias for rendering pipeline imports."""

from __future__ import annotations

import sys

from .rendering import highlight as _highlight

sys.modules[__name__] = _highlight
