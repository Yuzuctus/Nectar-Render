"""Tests for CLI parsing and execution."""

from __future__ import annotations

from pathlib import Path

import nectar_render.cli as cli_module
from nectar_render.services.conversion_service import ConversionResult


def test_state_dict_to_style_maps_tk_state_keys() -> None:
    style = cli_module._state_dict_to_style(
        {
            "footer_align_var": "Center",
            "image_scale_var": 80.0,
            "heading_h2_color_var": "#00ff00",
            "heading_h2_size_var": 20,
        }
    )

    assert style.footer_align == "center"
    assert style.image_scale == 0.8
    assert style.heading_h2_color == "#00ff00"
    assert style.heading_h2_size_px == 20


def test_run_cli_html_success_with_builtin_preset(
    monkeypatch, tmp_path: Path, capsys
) -> None:
    markdown_file = tmp_path / "demo.md"
    markdown_file.write_text("# Demo", encoding="utf-8")
    captured: dict[str, object] = {}

    class FakeService:
        def convert(
            self, markdown_file: Path, output_directory: Path, style, export
        ) -> ConversionResult:
            captured["markdown_file"] = markdown_file
            captured["output_directory"] = output_directory
            captured["style"] = style
            captured["export"] = export
            html_path = output_directory / "demo.html"
            return ConversionResult(html_path=html_path)

    monkeypatch.setattr(cli_module, "configure_logging", lambda: None)
    monkeypatch.setattr(cli_module, "prepare_weasyprint_environment", lambda: None)
    monkeypatch.setattr(cli_module, "ConversionService", FakeService)

    args = cli_module.build_parser().parse_args(
        [
            "--input",
            str(markdown_file),
            "--format",
            "html",
            "--page-size",
            "letter",
            "--preset",
            "academic",
        ]
    )

    assert cli_module.run_cli(args) == 0
    assert captured["markdown_file"] == markdown_file
    assert captured["output_directory"] == markdown_file.parent / "output"
    assert captured["style"].body_font == "Times New Roman"
    assert captured["style"].footer_align == "center"
    assert captured["export"].output_format == "HTML"
    assert captured["export"].page_size == "Letter"
    assert "Generated:" in capsys.readouterr().out


def test_run_cli_rejects_unknown_preset(monkeypatch, tmp_path: Path, capsys) -> None:
    markdown_file = tmp_path / "demo.md"
    markdown_file.write_text("# Demo", encoding="utf-8")

    monkeypatch.setattr(cli_module, "configure_logging", lambda: None)
    monkeypatch.setattr(cli_module, "prepare_weasyprint_environment", lambda: None)

    args = cli_module.build_parser().parse_args(
        ["--input", str(markdown_file), "--preset", "Modern"]
    )

    assert cli_module.run_cli(args) == 1
    error_output = capsys.readouterr().err
    assert "unknown preset 'Modern'" in error_output
    for name in cli_module.BUILTIN_PRESET_NAMES:
        assert name in error_output


def test_run_cli_hides_pdf_size_when_postprocess_not_applied(
    monkeypatch, tmp_path: Path, capsys
) -> None:
    markdown_file = tmp_path / "demo.md"
    markdown_file.write_text("# Demo", encoding="utf-8")

    class FakeService:
        def convert(
            self, markdown_file: Path, output_directory: Path, style, export
        ) -> ConversionResult:
            pdf_path = output_directory / "demo.pdf"
            return ConversionResult(
                pdf_path=pdf_path,
                pdf_page_count=3,
                pdf_size_before_bytes=10_000,
                pdf_size_after_bytes=10_000,
                pdf_compression_applied=False,
            )

    monkeypatch.setattr(cli_module, "configure_logging", lambda: None)
    monkeypatch.setattr(cli_module, "prepare_weasyprint_environment", lambda: None)
    monkeypatch.setattr(cli_module, "ConversionService", FakeService)

    args = cli_module.build_parser().parse_args(["--input", str(markdown_file)])

    assert cli_module.run_cli(args) == 0
    output = capsys.readouterr().out
    assert "PDF pages: 3" in output
    assert "PDF size:" not in output
