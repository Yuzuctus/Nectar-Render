from __future__ import annotations

from pathlib import Path

from nectar_render.config import ExportOptions, StyleOptions
import nectar_render.ui.controllers as controllers_module


class ImmediateRoot:
    def __init__(self) -> None:
        self.after_calls: list[tuple[int, object, tuple[object, ...]]] = []

    def after(self, delay_ms: int, callback: object, *args: object) -> str:
        self.after_calls.append((delay_ms, callback, args))
        callback(*args)  # type: ignore[misc]
        return f"after-{len(self.after_calls)}"


class ImmediateThread:
    def __init__(self, *, target, daemon: bool) -> None:
        self._target = target
        self.daemon = daemon

    def start(self) -> None:
        self._target()


def test_preview_html_opens_generated_file_and_updates_status(
    monkeypatch, tmp_path: Path
) -> None:
    markdown_file = tmp_path / "demo.md"
    markdown_file.write_text("# Demo", encoding="utf-8")
    opened: list[tuple[str, int]] = []
    statuses: list[str] = []
    captured: dict[str, object] = {}

    class FakeService:
        def convert(
            self,
            *,
            markdown_file: Path,
            output_directory: Path,
            style: StyleOptions,
            export: ExportOptions,
        ) -> controllers_module.ConversionResult:
            captured["markdown_file"] = markdown_file
            captured["output_directory"] = output_directory
            captured["style"] = style
            captured["export"] = export
            html_path = output_directory / "demo.html"
            return controllers_module.ConversionResult(html_path=html_path)

    monkeypatch.setattr(
        controllers_module.webbrowser,
        "open",
        lambda url, new=0: opened.append((url, new)),
    )

    controller = controllers_module.PreviewController(
        FakeService(),
        require_markdown_path=lambda: markdown_file,
        collect_style=lambda: StyleOptions(footer_text="Preview"),
        configured_output_dir_getter=lambda: "",
        page_size_getter=lambda: " Letter ",
        heading_level_detector=lambda _text: 3,
        set_status=statuses.append,
    )

    controller.preview_html()

    expected_output_dir = markdown_file.parent / "output"
    expected_html_path = expected_output_dir / "demo.html"
    assert captured["markdown_file"] == markdown_file
    assert captured["output_directory"] == expected_output_dir
    assert isinstance(captured["style"], StyleOptions)
    assert captured["export"] == ExportOptions(output_format="HTML", page_size="Letter")
    assert opened == [(expected_html_path.as_uri(), 0)]
    assert statuses == [f"Preview: {expected_html_path}"]


def test_preview_html_reports_failures(monkeypatch, tmp_path: Path) -> None:
    markdown_file = tmp_path / "demo.md"
    markdown_file.write_text("# Demo", encoding="utf-8")
    errors: list[tuple[str, str]] = []
    statuses: list[str] = []

    class FailingService:
        def convert(self, **_: object) -> controllers_module.ConversionResult:
            raise RuntimeError("boom")

    monkeypatch.setattr(
        controllers_module.messagebox,
        "showerror",
        lambda title, message: errors.append((title, message)),
    )

    controller = controllers_module.PreviewController(
        FailingService(),
        require_markdown_path=lambda: markdown_file,
        collect_style=lambda: StyleOptions(),
        configured_output_dir_getter=lambda: "",
        page_size_getter=lambda: "A4",
        heading_level_detector=lambda _text: 3,
        set_status=statuses.append,
    )

    controller.preview_html()

    assert errors == [("Error", "Cannot preview: boom")]
    assert statuses == ["Preview failed"]


def test_preview_test_html_writes_preview_file_and_opens_browser(
    monkeypatch, tmp_path: Path
) -> None:
    markdown_file = tmp_path / "demo.md"
    markdown_file.write_text("# Demo", encoding="utf-8")
    opened: list[tuple[Path, bool]] = []
    statuses: list[str] = []

    monkeypatch.setattr(
        controllers_module,
        "build_style_preview_document",
        lambda **_: ("<html>preview</html>", 5),
    )

    controller = controllers_module.PreviewController(
        service=object(),  # type: ignore[arg-type]
        require_markdown_path=lambda: markdown_file,
        collect_style=lambda: StyleOptions(),
        configured_output_dir_getter=lambda: str(tmp_path / "custom-output"),
        page_size_getter=lambda: "A4",
        heading_level_detector=lambda _text: 3,
        set_status=statuses.append,
    )
    monkeypatch.setattr(
        controller,
        "_open_browser_preview",
        lambda preview_path, force_refresh: opened.append(
            (preview_path, force_refresh)
        ),
    )

    controller.preview_test_html()

    preview_path = tmp_path / "custom-output" / "_style_preview_test.html"
    assert preview_path.read_text(encoding="utf-8") == "<html>preview</html>"
    assert opened == [(preview_path, True)]
    assert statuses == ["Test preview generated: H1-H5"]


def test_start_conversion_runs_service_and_reports_success(
    monkeypatch, tmp_path: Path
) -> None:
    markdown_file = tmp_path / "demo.md"
    markdown_file.write_text("# Demo", encoding="utf-8")
    statuses: list[str] = []
    enabled_states: list[bool] = []
    infos: list[tuple[str, str]] = []
    captured: dict[str, object] = {}
    root = ImmediateRoot()

    class FakeService:
        def convert(
            self,
            *,
            markdown_file: Path,
            output_directory: Path,
            style: StyleOptions,
            export: ExportOptions,
        ) -> controllers_module.ConversionResult:
            captured["markdown_file"] = markdown_file
            captured["output_directory"] = output_directory
            captured["style"] = style
            captured["export"] = export
            return controllers_module.ConversionResult(
                html_path=output_directory / "demo.html",
                pdf_path=output_directory / "demo.pdf",
                pdf_page_count=2,
                pdf_size_before_bytes=1024,
                pdf_size_after_bytes=512,
            )

    monkeypatch.setattr(
        controllers_module.threading,
        "Thread",
        lambda *, target, daemon: ImmediateThread(target=target, daemon=daemon),
    )
    monkeypatch.setattr(
        controllers_module.messagebox,
        "showinfo",
        lambda title, message: infos.append((title, message)),
    )

    controller = controllers_module.ConversionController(
        root,
        FakeService(),
        require_markdown_path=lambda: markdown_file,
        collect_style=lambda: StyleOptions(),
        collect_export=lambda: ExportOptions(output_format="PDF+HTML"),
        configured_output_dir_getter=lambda: "",
        set_status=statuses.append,
        set_convert_enabled=enabled_states.append,
    )

    controller.start_conversion()

    expected_output_dir = markdown_file.parent / "output"
    assert captured["markdown_file"] == markdown_file
    assert captured["output_directory"] == expected_output_dir
    assert isinstance(captured["style"], StyleOptions)
    assert captured["export"] == ExportOptions(output_format="PDF+HTML")
    assert enabled_states == [False, True]
    assert statuses == [
        "Converting...",
        "Conversion complete (2 pages) | PDF 1.0KB -> 0.5KB",
    ]
    assert infos == [
        (
            "Success",
            "Generated files:\n"
            f"{expected_output_dir / 'demo.pdf'}\n"
            f"{expected_output_dir / 'demo.html'}\n"
            "PDF pages: 2\n"
            "PDF size: 1.0KB -> 0.5KB (50.0%)",
        )
    ]


def test_start_conversion_reports_weasyprint_runtime_errors(
    monkeypatch, tmp_path: Path
) -> None:
    markdown_file = tmp_path / "demo.md"
    markdown_file.write_text("# Demo", encoding="utf-8")
    statuses: list[str] = []
    enabled_states: list[bool] = []
    errors: list[tuple[str, str]] = []
    root = ImmediateRoot()

    class FailingService:
        def convert(self, **_: object) -> controllers_module.ConversionResult:
            raise RuntimeError("cannot load library 'pango-1.0-0'")

    monkeypatch.setattr(
        controllers_module.threading,
        "Thread",
        lambda *, target, daemon: ImmediateThread(target=target, daemon=daemon),
    )
    monkeypatch.setattr(
        controllers_module.messagebox,
        "showerror",
        lambda title, message: errors.append((title, message)),
    )

    controller = controllers_module.ConversionController(
        root,
        FailingService(),
        require_markdown_path=lambda: markdown_file,
        collect_style=lambda: StyleOptions(),
        collect_export=lambda: ExportOptions(output_format="PDF"),
        configured_output_dir_getter=lambda: "",
        set_status=statuses.append,
        set_convert_enabled=enabled_states.append,
    )

    controller.start_conversion()

    assert enabled_states == [False, True]
    assert statuses == ["Converting...", "Conversion failed"]
    assert errors == [
        (
            "Conversion error",
            "cannot load library 'pango-1.0-0'\n\n"
            "On Windows, check the Pango/MSYS2 installation for WeasyPrint (see README).",
        )
    ]
