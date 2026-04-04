"""Compatibility alias for desktop controller imports."""

from __future__ import annotations

import sys

from ..interfaces.desktop import controllers as _controllers

sys.modules[__name__] = _controllers
