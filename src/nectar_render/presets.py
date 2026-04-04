"""Compatibility alias for desktop-state preset imports."""

from __future__ import annotations

import sys

from .interfaces.desktop import presets as _presets

sys.modules[__name__] = _presets
