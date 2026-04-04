"""Compatibility alias for the legacy desktop app module path."""

from __future__ import annotations

import sys

from ..interfaces.desktop import app as _app

sys.modules[__name__] = _app
