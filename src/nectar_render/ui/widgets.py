"""Compatibility alias for desktop widget imports."""

from __future__ import annotations

import sys

from ..interfaces.desktop import widgets as _widgets

sys.modules[__name__] = _widgets
