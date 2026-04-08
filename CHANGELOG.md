# Changelog

## Unreleased

### Changed
- Removed Tkinter desktop GUI
- Removed legacy service shims (`ui/`, `services/`, `converter/`, `interfaces/`)
- Renamed `interfaces/core/` → `core/`
- Added `ImageMode` enum for flexible PDF export (`WITH_IMAGES`, `ALT_ONLY`, `STRIP`)
- Added `analyze_markdown()` for pre-flight image analysis
- Added `execute_conversion()` support for memory-based rendering via `markdown_text` and `output_bytes`
- Added thread safety, early exit, and warning logging to markdown pipeline

### Added
- `extract_referenced_images()` function in `adapters/rendering/markdown_pipeline.py`
- `missing_images` field in `ConversionResult` for tracking absent assets

### Fixed
- Stopped promoting post-processed PDFs when qpdf or metadata cleanup would make the file larger.
