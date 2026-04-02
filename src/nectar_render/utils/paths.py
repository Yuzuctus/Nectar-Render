from __future__ import annotations

import os
import sys
from pathlib import Path


def application_data_dir() -> Path:
    if sys.platform == "win32":
        appdata = os.environ.get("APPDATA", "").strip()
        base_dir = Path(appdata) if appdata else (Path.home() / "AppData" / "Roaming")
        return base_dir / "nectar-render"
    return Path.home() / ".nectar-render"


def default_output_dir(markdown_file: Path | None) -> Path:
    if markdown_file is None:
        documents_dir = Path.home() / "Documents"
        base_dir = documents_dir if documents_dir.exists() else Path.home()
        return base_dir / "Nectar Render" / "output"
    return markdown_file.parent / "output"
