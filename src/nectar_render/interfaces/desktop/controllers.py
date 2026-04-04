from __future__ import annotations

import logging
import threading
import time
import tkinter as tk
import webbrowser
from collections.abc import Callable
from pathlib import Path
from tkinter import messagebox

from ...application.conversion import ConversionResult, ConversionService
from ...application.preview import (
    build_style_preview_document,
    resolve_output_directory,
)
from ...core.styles import ExportOptions, StyleOptions


logger = logging.getLogger(__name__)


class PreviewController:
    def __init__(
        self,
        service: ConversionService,
        *,
        require_markdown_path: Callable[[], Path | None],
        collect_style: Callable[[], StyleOptions],
        configured_output_dir_getter: Callable[[], str],
        page_size_getter: Callable[[], str],
        heading_level_detector: Callable[[str], int],
        set_status: Callable[[str], None],
    ) -> None:
        self.service = service
        self.require_markdown_path = require_markdown_path
        self.collect_style = collect_style
        self.configured_output_dir_getter = configured_output_dir_getter
        self.page_size_getter = page_size_getter
        self.heading_level_detector = heading_level_detector
        self.set_status = set_status

    def preview_html(self) -> None:
        md_path = self.require_markdown_path()
        if md_path is None:
            return
        try:
            style = self.collect_style()
            export = ExportOptions(output_format="HTML", page_size=self._page_size())
            result = self.service.convert(
                markdown_file=md_path,
                output_directory=resolve_output_directory(
                    md_path,
                    self.configured_output_dir_getter(),
                ),
                style=style,
                export=export,
            )
            if result.html_path:
                webbrowser.open(result.html_path.as_uri())
                self.set_status(f"Preview: {result.html_path}")
        except Exception as exc:
            logger.exception("HTML preview failed")
            messagebox.showerror("Error", f"Cannot preview: {exc}")
            self.set_status("Preview failed")

    def preview_test_html(self) -> None:
        md_path = self.require_markdown_path()
        if md_path is None:
            return
        try:
            style = self.collect_style()
            source_markdown = md_path.read_text(encoding="utf-8")
            html, max_heading = build_style_preview_document(
                markdown_path=md_path,
                style=style,
                page_size=self._page_size(),
                source_markdown=source_markdown,
                heading_level_detector=self.heading_level_detector,
            )

            output_dir = resolve_output_directory(
                md_path,
                self.configured_output_dir_getter(),
            )
            output_dir.mkdir(parents=True, exist_ok=True)
            preview_path = output_dir / "_style_preview_test.html"
            preview_path.write_text(html, encoding="utf-8")

            self._open_browser_preview(preview_path, force_refresh=True)
            self.set_status(f"Test preview generated: H1-H{max_heading}")
        except Exception as exc:
            logger.exception("Test preview generation failed")
            messagebox.showerror("Test preview error", str(exc))
            self.set_status("Test preview generation failed")

    def _page_size(self) -> str:
        return self.page_size_getter().strip() or "A4"

    @staticmethod
    def _open_browser_preview(preview_path: Path, force_refresh: bool) -> None:
        preview_url = preview_path.as_uri()
        if force_refresh:
            preview_url = f"{preview_url}?v={time.time_ns()}"
        webbrowser.open(preview_url, new=0)


class ConversionController:
    def __init__(
        self,
        root: tk.Misc,
        service: ConversionService,
        *,
        require_markdown_path: Callable[[], Path | None],
        collect_style: Callable[[], StyleOptions],
        collect_export: Callable[[], ExportOptions],
        configured_output_dir_getter: Callable[[], str],
        set_status: Callable[[str], None],
        set_convert_enabled: Callable[[bool], None],
    ) -> None:
        self.root = root
        self.service = service
        self.require_markdown_path = require_markdown_path
        self.collect_style = collect_style
        self.collect_export = collect_export
        self.configured_output_dir_getter = configured_output_dir_getter
        self.set_status = set_status
        self.set_convert_enabled = set_convert_enabled

    def start_conversion(self) -> None:
        md_path = self.require_markdown_path()
        if md_path is None:
            return

        configured_output_dir = self.configured_output_dir_getter().strip()
        output_dir = resolve_output_directory(
            md_path,
            configured_output_dir,
        )
        style = self.collect_style()
        export = self.collect_export()

        self.set_status("Converting...")
        self.set_convert_enabled(False)

        def _run() -> None:
            try:
                result = self.service.convert(
                    markdown_file=md_path,
                    output_directory=output_dir,
                    style=style,
                    export=export,
                )
                self.root.after(0, self._on_convert_success, result)
            except Exception as exc:
                logger.exception("Conversion failed")
                self.root.after(0, self._on_convert_error, exc)

        threading.Thread(target=_run, daemon=True).start()

    def _on_convert_success(self, result: ConversionResult) -> None:
        self.set_convert_enabled(True)
        produced = [str(path) for path in [result.pdf_path, result.html_path] if path]
        page_info = ""
        if result.pdf_page_count is not None:
            page_info = f"\nPDF pages: {result.pdf_page_count}"

        compression_info = ""
        if (
            result.pdf_size_before_bytes is not None
            and result.pdf_size_after_bytes is not None
        ):
            before_kb = result.pdf_size_before_bytes / 1024
            after_kb = result.pdf_size_after_bytes / 1024
            gain_pct = 0.0
            if result.pdf_size_before_bytes > 0:
                gain_pct = (
                    (result.pdf_size_before_bytes - result.pdf_size_after_bytes)
                    / result.pdf_size_before_bytes
                ) * 100
            compression_info = (
                f"\nPDF size: {before_kb:.1f}KB -> {after_kb:.1f}KB ({gain_pct:.1f}%)"
            )
            if (
                result.pdf_size_after_bytes >= result.pdf_size_before_bytes
                and not result.pdf_compression_tool
            ):
                compression_info += (
                    "\nTip: install qpdf for better compression on some documents."
                )

        status_msg = "Conversion complete"
        if result.pdf_page_count is not None:
            status_msg += f" ({result.pdf_page_count} pages)"
        if result.pdf_size_before_bytes and result.pdf_size_after_bytes:
            before_kb = result.pdf_size_before_bytes / 1024
            after_kb = result.pdf_size_after_bytes / 1024
            status_msg += f" | PDF {before_kb:.1f}KB -> {after_kb:.1f}KB"
        self.set_status(status_msg)

        messagebox.showinfo(
            "Success",
            "Generated files:\n" + "\n".join(produced) + page_info + compression_info,
        )

    def _on_convert_error(self, exc: Exception) -> None:
        self.set_convert_enabled(True)
        message = str(exc)
        if "cannot load library" in message.lower() or "pango" in message.lower():
            message += "\n\nOn Windows, check the Pango/MSYS2 installation for WeasyPrint (see README)."
        messagebox.showerror("Conversion error", message)
        self.set_status("Conversion failed")
