# Changelog

## Unreleased

- Refactored the project toward a `core` / `application` / `adapters` / `interfaces` architecture.
- Moved the active Tkinter desktop implementation under `interfaces/desktop`.
- Moved the active Markdown/HTML/PDF rendering pipeline under `adapters/rendering`.
- Stopped promoting post-processed PDFs when qpdf or metadata cleanup would make the file larger.
- Kept legacy `ui/`, `services/`, `presets.py`, and `style_schema.py` as compatibility layers during migration.
- Kept `converter/` as a compatibility layer while the public rendering imports remain supported.
- Updated the README to reflect the current structure, CLI usage, and legacy compatibility status.
