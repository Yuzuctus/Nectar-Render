"""Compatibility alias for rendering pipeline imports."""

from __future__ import annotations

import sys

from .rendering import document_renderer as _document_renderer

sys.modules[__name__] = _document_renderer
