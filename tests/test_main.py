from __future__ import annotations

import importlib
import importlib.metadata
import json
import subprocess
import sys
from pathlib import Path

import pytest

import nectar_render


def test_version_matches_installed_distribution() -> None:
    assert nectar_render.__version__ == importlib.metadata.version("nectar-render")


def _fresh_import_loaded_modules(module_name: str) -> list[str]:
    probe = (
        "import json, sys; "
        f"import {module_name}; "
        "blocked = ("
        "'nectar_render.interfaces.desktop.app',"
        "'nectar_render.interfaces.desktop.app_state',"
        "'nectar_render.interfaces.desktop.bindings',"
        "'nectar_render.interfaces.desktop.controllers',"
        "'nectar_render.interfaces.desktop.panels',"
        "'nectar_render.interfaces.desktop.state_manager',"
        "'nectar_render.interfaces.desktop.theme',"
        "'nectar_render.interfaces.desktop.widgets'"
        "); "
        "mods = sorted("
        "name for name in sys.modules "
        "if name == 'tkinter' or name.startswith('tkinter.') "
        "or name.startswith('nectar_render.ui') "
        "or name in blocked"
        "); "
        "print(json.dumps(mods))"
    )
    completed = subprocess.run(
        [sys.executable, "-c", probe],
        capture_output=True,
        text=True,
        check=True,
    )
    return json.loads(completed.stdout)


def test_importing_main_does_not_import_gui_modules() -> None:
    loaded = _fresh_import_loaded_modules("nectar_render.main")
    assert loaded == []


def test_importing_cli_does_not_import_gui_modules() -> None:
    loaded = _fresh_import_loaded_modules("nectar_render.cli")
    assert loaded == []


def test_cli_path_dispatches_without_importing_gui(monkeypatch, tmp_path: Path) -> None:
    markdown_file = tmp_path / "demo.md"
    markdown_file.write_text("# Demo", encoding="utf-8")

    sys.modules.pop("nectar_render.main", None)
    sys.modules.pop("nectar_render.ui.app", None)
    sys.modules.pop("nectar_render.interfaces.desktop.app", None)
    sys.modules.pop("tkinter", None)

    main_module = importlib.import_module("nectar_render.main")
    monkeypatch.setattr(
        main_module, "_run_gui", lambda: pytest.fail("GUI path should not run")
    )
    monkeypatch.setattr(main_module, "run_cli", lambda args: 0)

    assert main_module.main(["--input", str(markdown_file)]) == 0
    assert "nectar_render.ui.app" not in sys.modules
    assert "nectar_render.interfaces.desktop.app" not in sys.modules
    assert "tkinter" not in sys.modules


def test_unknown_cli_argument_raises_standard_argparse_exit(tmp_path: Path) -> None:
    markdown_file = tmp_path / "demo.md"
    markdown_file.write_text("# Demo", encoding="utf-8")

    from nectar_render.main import main

    with pytest.raises(SystemExit) as exc:
        main(["--input", str(markdown_file), "--bogus"])
    assert exc.value.code == 2
