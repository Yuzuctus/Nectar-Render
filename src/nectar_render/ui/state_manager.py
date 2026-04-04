"""Compatibility alias for desktop state manager imports."""

from __future__ import annotations

import sys

from ..interfaces.desktop import state_manager as _state_manager

sys.modules[__name__] = _state_manager
