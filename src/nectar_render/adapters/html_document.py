"""Compatibility alias for rendering pipeline imports."""

from __future__ import annotations

import sys

from .rendering import html_document as _html_document

sys.modules[__name__] = _html_document
