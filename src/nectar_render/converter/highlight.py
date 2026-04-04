"""Compatibility alias for legacy syntax highlight imports."""

from __future__ import annotations

import sys

from ..adapters.rendering import highlight as _highlight

sys.modules[__name__] = _highlight
