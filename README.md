# Nectar Render

Desktop application to convert Markdown files to styled PDF and HTML with browser preview, syntax highlighting, and preset themes.

![Python](https://img.shields.io/badge/python-%3E%3D3.10-blue)
![License](https://img.shields.io/badge/license-PolyForm%20NC%201.0.0-orange)

<!-- ![Screenshot](docs/screenshot.png) -->

## Features

- Convert Markdown to **PDF**, **HTML**, or both in one pass
- Tkinter GUI with light and dark themes
- **9 built-in presets**: Academic, Magazine, Corporate, Technical, Minimal, Notebook, Creative, Developer, Elegant
- Syntax highlighting for code blocks (7+ Pygments themes)
- Full typography control: fonts, sizes, colors for body, headings (H1-H6), and code
- Page break markers: `<!-- pagebreak -->`, `\pagebreak`, `[[PAGEBREAK]]`
- Smart pagination to avoid orphaned headings and split tables
- Footnotes compatible with PDF rendering (CSS `float: footnote`)
- Obsidian image embeds: `![[image.png]]`
- Optional PDF compression via `qpdf`
- Save and load custom presets
- Undo/redo for all style changes (Ctrl+Z / Ctrl+Alt+Z)

## Architecture

The current codebase is organized in four layers:

- `core/` contains the canonical style and preset models.
- `application/` contains use cases such as conversion and preview.
- `adapters/` contains rendering, storage, and runtime integration.
- `adapters/rendering/` is the canonical Markdown -> HTML -> PDF pipeline.
- `interfaces/desktop/` contains the Tkinter application, widgets, controllers, and state mapping.

The legacy packages `ui/`, `converter/`, `services/`, `presets.py`, and `style_schema.py` are compatibility shims kept for now so older imports still work. New code should use the canonical layers above.

## Quick Start

### Windows (automated)

```powershell
.\launch.bat
```

This script creates a virtual environment, installs dependencies, and launches the GUI.

### Manual installation

```bash
python -m venv .venv
# Windows:
.\.venv\Scripts\Activate.ps1
# Linux/macOS:
source .venv/bin/activate

pip install -e ".[dev]"
nectar-render
```

Or run directly:

```bash
python -m nectar_render.main
```

## CLI

The CLI is the simplest way to run conversions without opening the desktop app.

```bash
nectar-render --input examples/sample.md --format pdf
nectar-render --input examples/sample.md --format html
nectar-render --input examples/sample.md --format pdf+html --preset Academic
```

Available options:

- `--input`, `-i`: Markdown input file
- `--output`, `-o`: Output directory
- `--format`, `-f`: `pdf`, `html`, or `pdf+html`
- `--page-size`: `A4`, `Letter`, `Legal`, `A3`, `A5`
- `--preset`, `-p`: built-in style preset
- `--no-compression`: disable PDF compression

The CLI uses the same application layer and rendering pipeline as the desktop app.

## Built-in Presets

Academic, Magazine, Corporate, Technical, Minimal, Notebook, Creative, Developer, and Elegant are available out of the box.

## WeasyPrint on Windows

HTML export works out of the box. PDF export requires native GTK/Pango libraries.

The app auto-detects common installation paths:

- `C:\msys64\ucrt64\bin`
- `C:\msys64\mingw64\bin`
- `WEASYPRINT_DLL_DIRECTORIES` environment variable

If you see errors like `cannot load library 'libgobject-2.0-0'`, install MSYS2 and Pango:

```powershell
winget install --id MSYS2.MSYS2 --accept-package-agreements --accept-source-agreements
C:\msys64\usr\bin\bash.exe -lc "pacman -S --needed mingw-w64-ucrt-x86_64-pango"
```

## PDF Compression

`qpdf` is optional but recommended for smaller PDF files:

```powershell
winget install --id qpdf.qpdf
```

Two profiles are available: **balanced** (default) and **max**.
Post-processing is only kept when the resulting PDF does not grow beyond the original file size.

## Example

The repository includes a showcase file:

```
examples/sample.md
examples/assets/service-overview.svg
examples/assets/diagrams/sequence.svg
```

1. Launch the app
2. Open `examples/sample.md`
3. Select **PDF+HTML** format
4. Click **Convert**

## Desktop Status

The desktop application is the primary interface today. Tkinter-specific code lives under `interfaces/desktop/`.

The older `ui/` package is still present as a compatibility layer during the migration. It should be treated as legacy plumbing, not as the canonical home for new code.

The older `converter/` package is also legacy plumbing now. The active rendering implementation lives under `adapters/rendering/`.

## Tests and Verification

The repository includes unit and integration tests for the parser, renderer, CLI, PDF compression, and state management.

```bash
pytest -q
ruff check src tests
ruff format --check src tests
```

## Notes

- On Windows, PDF export depends on WeasyPrint and native GTK/Pango libraries.
- `qpdf` is optional but recommended for smaller PDFs.
- A future web interface is planned, and the current `core` / `application` split is intended to make that migration simpler.

## Contributing

1. Fork the repository
2. Create a feature branch
3. Run `pytest` before submitting
4. Open a pull request

## License

[PolyForm Noncommercial 1.0.0](LICENSE)

You may use, copy, modify, and redistribute this project for noncommercial purposes.
Commercial use is not permitted without separate permission from the author.

Versions that were already distributed under MIT before this change remain available
under MIT for those existing copies.
