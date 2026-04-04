"""Compatibility alias for legacy markdown pipeline imports."""

from __future__ import annotations

import sys

from ..adapters.rendering import markdown_pipeline as _markdown_pipeline

sys.modules[__name__] = _markdown_pipeline
