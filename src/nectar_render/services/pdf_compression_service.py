"""Compatibility facade for legacy PDF compression imports."""

from __future__ import annotations

from ..adapters.pdf_postprocess import PdfCompressionResult, PdfCompressionService

__all__ = ["PdfCompressionResult", "PdfCompressionService"]
