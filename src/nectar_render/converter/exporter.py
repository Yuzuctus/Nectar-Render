"""Compatibility alias for legacy rendering export imports."""

from __future__ import annotations

import sys

from ..adapters.rendering import pdf_export as _pdf_export

sys.modules[__name__] = _pdf_export
