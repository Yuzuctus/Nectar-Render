"""Compatibility alias for legacy HTML document builder imports."""

from __future__ import annotations

import sys

from ..adapters.rendering import html_document as _html_document

sys.modules[__name__] = _html_document
