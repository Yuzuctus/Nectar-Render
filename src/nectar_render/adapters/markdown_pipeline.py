"""Compatibility alias for rendering pipeline imports."""

from __future__ import annotations

import sys

from .rendering import markdown_pipeline as _markdown_pipeline

sys.modules[__name__] = _markdown_pipeline
