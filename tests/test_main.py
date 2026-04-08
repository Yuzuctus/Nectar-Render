from __future__ import annotations

import importlib
import importlib.metadata
import subprocess
import sys
from pathlib import Path

import pytest

import nectar_render


def test_version_matches_installed_distribution() -> None:
    assert nectar_render.__version__ == importlib.metadata.version("nectar-render")


def test_importing_main_does_not_import_gui_modules() -> None:
    probe = (
        "import sys; "
        "import nectar_render.main; "
        "mods = [n for n in sys.modules.keys() "
        "if 'tkinter' in n or 'nectar_render.ui' in n or 'nectar_render.interfaces.desktop' in n]; "
        "print(mods)"
    )
    completed = subprocess.run(
        [sys.executable, "-c", probe],
        capture_output=True,
        text=True,
        check=True,
    )
    assert completed.stdout.strip() == "[]"


def test_importing_cli_does_not_import_gui_modules() -> None:
    probe = (
        "import sys; "
        "import nectar_render.cli; "
        "mods = [n for n in sys.modules.keys() "
        "if 'tkinter' in n or 'nectar_render.ui' in n or 'nectar_render.interfaces.desktop' in n]; "
        "print(mods)"
    )
    completed = subprocess.run(
        [sys.executable, "-c", probe],
        capture_output=True,
        text=True,
        check=True,
    )
    assert completed.stdout.strip() == "[]"


def test_unknown_cli_argument_raises_standard_argparse_exit(tmp_path: Path) -> None:
    markdown_file = tmp_path / "demo.md"
    markdown_file.write_text("# Demo", encoding="utf-8")

    from nectar_render.main import main

    with pytest.raises(SystemExit) as exc:
        main(["--input", str(markdown_file), "--bogus"])
    assert exc.value.code == 2
