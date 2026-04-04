"""Compatibility alias for desktop theme imports."""

from __future__ import annotations

import sys

from ..interfaces.desktop import theme as _theme

sys.modules[__name__] = _theme
