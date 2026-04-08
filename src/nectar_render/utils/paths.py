from __future__ import annotations

import os
import re
import sys
from pathlib import Path
from urllib.parse import urlparse


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


def is_external_or_absolute_path(src: str) -> bool:
    """Check if a source path is external URL or absolute path.

    Returns True for paths that pose security risks (SSRF/LFI):
    - http:// or https:// URLs (external - SSRF risk)
    - data: URIs (inline data, flagged as external)
    - file:// URLs (LFI risk)
    - ftp:// URLs (external resource)
    - Absolute Unix paths (/etc/passwd, /var/...)
    - Absolute Windows paths (C:\\, D:/, \\\\share)

    Args:
        src: The source path or URL to check.

    Returns:
        True if the path is external or absolute, False if it's a relative path.
    """
    parsed = urlparse(src)
    # External URLs and special schemes
    if parsed.scheme and parsed.scheme.lower() in {
        "http",
        "https",
        "data",
        "file",
        "ftp",
    }:
        return True
    # Unix absolute paths
    if src.startswith("/") or src.startswith("\\"):
        return True
    # Windows absolute paths (C:\, D:/, etc.)
    if re.match(r"^[a-zA-Z]:[\\/]", src):
        return True
    # UNC paths (\\server\share)
    if src.startswith("\\\\"):
        return True
    return False
