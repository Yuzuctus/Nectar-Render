"""Compatibility alias for legacy desktop bindings imports."""

from __future__ import annotations

import sys

from ..interfaces.desktop import bindings as _bindings

sys.modules[__name__] = _bindings
