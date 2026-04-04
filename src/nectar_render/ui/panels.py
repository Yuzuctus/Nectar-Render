"""Compatibility alias for desktop panel imports."""

from __future__ import annotations

import sys

from ..interfaces.desktop import panels as _panels

sys.modules[__name__] = _panels
