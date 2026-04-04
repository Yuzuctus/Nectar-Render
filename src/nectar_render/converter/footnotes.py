"""Compatibility alias for legacy footnote imports."""

from __future__ import annotations

import sys

from ..adapters.rendering import footnotes as _footnotes

sys.modules[__name__] = _footnotes
