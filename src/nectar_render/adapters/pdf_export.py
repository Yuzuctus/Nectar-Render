"""Compatibility alias for rendering pipeline imports."""

from __future__ import annotations

import sys

from .rendering import pdf_export as _pdf_export

sys.modules[__name__] = _pdf_export
