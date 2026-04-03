"""Tests for state normalization and persistence behavior."""

from __future__ import annotations

import json
import tkinter as tk
from pathlib import Path

import nectar_render.ui.state_manager as state_manager_module
from nectar_render.ui.state_manager import StateManager


class FakeRoot:
    def __init__(self) -> None:
        self.after_calls: list[tuple[int, object, str]] = []
        self.cancelled_jobs: list[str] = []

    def after(self, delay_ms: int, callback: object) -> str:
        job_id = f"job-{len(self.after_calls) + 1}"
        self.after_calls.append((delay_ms, callback, job_id))
        return job_id

    def after_cancel(self, job_id: str) -> None:
        self.cancelled_jobs.append(job_id)


def _build_state_manager(
    tmp_path: Path,
) -> tuple[StateManager, FakeRoot, dict[str, tk.Variable], tk.StringVar, list[str]]:
    interpreter = tk.Tcl()
    root = FakeRoot()
    manager = StateManager(
        root=root,  # type: ignore[arg-type]
        app_dir=tmp_path,
        presets_filename="presets.json",
        state_filename="last_state.json",
    )
    status_var = tk.StringVar(master=interpreter, value="Ready")
    ensured_fonts: list[str] = []
    tk_vars: dict[str, tk.Variable] = {
        "body_font_var": tk.StringVar(master=interpreter, value="Segoe UI"),
        "heading_font_var": tk.StringVar(master=interpreter, value="Segoe UI"),
        "code_font_var": tk.StringVar(master=interpreter, value="Consolas"),
        "format_var": tk.StringVar(master=interpreter, value="PDF"),
        "page_size_var": tk.StringVar(master=interpreter, value="A4"),
        "ui_theme_var": tk.StringVar(master=interpreter, value="Light"),
        "pdf_compression_profile_var": tk.StringVar(
            master=interpreter, value="balanced"
        ),
        "footer_align_var": tk.StringVar(master=interpreter, value="Right"),
        "code_theme_var": tk.StringVar(master=interpreter, value="default"),
        "body_size_var": tk.IntVar(master=interpreter, value=12),
        "heading_h1_size_var": tk.IntVar(master=interpreter, value=28),
        "line_height_var": tk.DoubleVar(master=interpreter, value=1.5),
        "margin_top_var": tk.DoubleVar(master=interpreter, value=25.4),
        "footnote_size_var": tk.DoubleVar(master=interpreter, value=9.0),
        "image_scale_var": tk.DoubleVar(master=interpreter, value=90.0),
    }
    manager.set_tk_vars(tk_vars, status_var, ensured_fonts.append)
    return manager, root, tk_vars, status_var, ensured_fonts


def test_apply_state_normalizes_enums_and_clamps_numeric_values(tmp_path: Path) -> None:
    manager, root, tk_vars, _status_var, ensured_fonts = _build_state_manager(tmp_path)

    manager.apply_state(
        {
            "body_font_var": " Aptos\nMono ",
            "heading_font_var": '"; invalid',
            "code_font_var": " JetBrains Mono ",
            "format_var": "pdf+html",
            "page_size_var": "letter",
            "ui_theme_var": "dark",
            "pdf_compression_profile_var": "MAX",
            "footer_align_var": "centre",
            "code_theme_var": "not-a-theme",
            "body_size_var": 2,
            "heading_h1_size_var": 999,
            "line_height_var": 9.9,
            "margin_top_var": -5,
            "footnote_size_var": 2,
            "image_scale_var": 250,
        }
    )

    assert ensured_fonts == ["Aptos Mono", '"; invalid', "JetBrains Mono"]
    assert tk_vars["body_font_var"].get() == "Aptos Mono"
    assert tk_vars["heading_font_var"].get() == '"; invalid'
    assert tk_vars["code_font_var"].get() == "JetBrains Mono"
    assert tk_vars["format_var"].get() == "PDF+HTML"
    assert tk_vars["page_size_var"].get() == "Letter"
    assert tk_vars["ui_theme_var"].get() == "Dark"
    assert tk_vars["pdf_compression_profile_var"].get() == "max"
    assert tk_vars["footer_align_var"].get() == "Center"
    assert tk_vars["code_theme_var"].get() == "default"
    assert tk_vars["body_size_var"].get() == 8
    assert tk_vars["heading_h1_size_var"].get() == 96
    assert tk_vars["line_height_var"].get() == 2.4
    assert tk_vars["margin_top_var"].get() == 0.0
    assert tk_vars["footnote_size_var"].get() == 7.0
    assert tk_vars["image_scale_var"].get() == 100.0
    assert root.after_calls[-1][0] == state_manager_module.PERSIST_DEBOUNCE_MS


def test_apply_state_preserves_previous_value_for_invalid_enums(tmp_path: Path) -> None:
    manager, _root, tk_vars, _status_var, _ensured_fonts = _build_state_manager(
        tmp_path
    )

    manager.apply_state(
        {
            "format_var": "word",
            "page_size_var": "poster",
            "ui_theme_var": "sepia",
            "pdf_compression_profile_var": "ultra",
            "code_theme_var": "not-real",
        }
    )

    assert tk_vars["format_var"].get() == "PDF"
    assert tk_vars["page_size_var"].get() == "A4"
    assert tk_vars["ui_theme_var"].get() == "Light"
    assert tk_vars["pdf_compression_profile_var"].get() == "balanced"
    assert tk_vars["code_theme_var"].get() == "default"


def test_load_last_state_ignores_invalid_json(tmp_path: Path) -> None:
    manager, _root, tk_vars, _status_var, _ensured_fonts = _build_state_manager(
        tmp_path
    )
    manager.state_file.write_bytes(b"{not-json")

    manager.load_last_state()

    assert tk_vars["page_size_var"].get() == "A4"
    assert tk_vars["format_var"].get() == "PDF"


def test_load_selected_user_preset_normalizes_untrusted_state(
    monkeypatch, tmp_path: Path
) -> None:
    manager, _root, tk_vars, status_var, _ensured_fonts = _build_state_manager(tmp_path)
    monkeypatch.setattr(
        state_manager_module.messagebox, "showwarning", lambda *args, **kwargs: None
    )
    manager.presets_file.write_text(
        json.dumps(
            {
                "presets": {
                    "Broken": {
                        "page_size_var": "bogus",
                        "format_var": "html",
                        "footer_align_var": "centre",
                        "body_size_var": 99,
                        "image_scale_var": 5,
                    }
                }
            }
        ),
        encoding="utf-8",
    )

    manager.load_selected_preset("Broken", status_var)

    assert tk_vars["page_size_var"].get() == "A4"
    assert tk_vars["format_var"].get() == "HTML"
    assert tk_vars["footer_align_var"].get() == "Center"
    assert tk_vars["body_size_var"].get() == 24
    assert tk_vars["image_scale_var"].get() == 40.0
    assert status_var.get() == "Preset loaded: Broken"
