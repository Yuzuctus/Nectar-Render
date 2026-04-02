# Nectar Render

> Desktop application to convert Markdown files to styled PDF and HTML with live preview, syntax highlighting, and preset themes.

![Python](https://img.shields.io/badge/python-%3E%3D3.10-blue)
![License](https://img.shields.io/badge/license-PolyForm%20NC%201.0.0-orange)

<!-- ![Screenshot](docs/screenshot.png) -->

## Features

- Convert Markdown to **PDF**, **HTML**, or both in one pass
- Tkinter GUI with light and dark themes
- **5 built-in presets**: Academic, Modern, Technical, Minimal, Dark Code
- Syntax highlighting for code blocks (7+ Pygments themes)
- Full typography control: fonts, sizes, colors for body, headings (H1-H6), and code
- Page break markers: `<!-- pagebreak -->`, `\pagebreak`, `[[PAGEBREAK]]`
- Smart pagination to avoid orphaned headings and split tables
- Footnotes compatible with PDF rendering (CSS `float: footnote`)
- Obsidian image embeds: `![[image.png]]`
- Optional PDF compression via `qpdf`
- Save and load custom presets
- Undo/redo for all style changes (Ctrl+Z / Ctrl+Alt+Z)

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

## Built-in Presets

| Preset | Body Font | Code Theme | Margins | Style |
|--------|-----------|------------|---------|-------|
| **Academic** | Times New Roman | friendly | 30mm | Serif, formal, generous spacing |
| **Modern** | Segoe UI | default | 25.4mm | Sans-serif, blue headings, striped tables |
| **Technical** | Calibri | monokai | 20mm | Compact, green accents, dense layout |
| **Minimal** | Arial | default | 15mm | Bare bones, maximum content space |
| **Dark Code** | Segoe UI | dracula | 25.4mm | Dark code blocks, blue heading accents |

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

## Tests

```bash
pytest -q
```

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
