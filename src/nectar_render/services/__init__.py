"""Compatibility exports for legacy service imports."""

from .conversion_service import ConversionResult, ConversionService
from .pdf_compression_service import PdfCompressionResult, PdfCompressionService

__all__ = [
    "ConversionResult",
    "ConversionService",
    "PdfCompressionResult",
    "PdfCompressionService",
]
