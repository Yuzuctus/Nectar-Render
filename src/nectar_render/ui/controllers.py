from __future__ import annotations

import logging
import re
import threading
import time
import tkinter as tk
import webbrowser
from collections.abc import Callable
from pathlib import Path
from tkinter import messagebox

from ..config import ExportOptions, StyleOptions
from ..converter.exporter import build_html_from_markdown
from ..services.conversion_service import ConversionResult, ConversionService
from ..utils.paths import default_output_dir


logger = logging.getLogger(__name__)

_SANDBOX_MARKDOWN = (
    "# Heading Level 1\n\n"
    "## Heading Level 2\n\n"
    "---\n\n"
    "### Heading Level 3\n\n"
    "Demo paragraph with `inline code` and a footnote[^note].\n\n"
    "| Column A | Column B |\n"
    "|---|---|\n"
    "| Value 1 | Value 2 |\n"
    "| Value 3 | Value 4 |\n\n"
    '```python\ndef greet(name: str) -> str:\n    return f"Hello {name}"\n\n'
    'print(greet("world"))\n```\n\n'
    "[^note]: Auto-generated footnote example.\n"
)


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
        self.live_preview_path: Path | None = None

    def preview_html(self) -> None:
        md_path = self.require_markdown_path()
        if md_path is None:
            return
        try:
            style = self.collect_style()
            export = ExportOptions(output_format="HTML", page_size=self._page_size())
            result = self.service.convert(
                markdown_file=md_path,
                output_directory=self._resolve_output_dir(md_path),
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
            markdown_text = self._build_generated_test_markdown(source_markdown)
            html = build_html_from_markdown(
                markdown_text=markdown_text,
                style=style,
                page_size=self._page_size(),
                title="Style test preview",
                assets_root=md_path.parent,
            )

            output_dir = self._resolve_output_dir(md_path)
            output_dir.mkdir(parents=True, exist_ok=True)
            preview_path = output_dir / "_style_preview_test.html"
            preview_path.write_text(html, encoding="utf-8")
            self.live_preview_path = preview_path

            self._open_browser_preview(preview_path, force_refresh=True)
            max_heading = max(1, min(6, self.heading_level_detector(source_markdown)))
            self.set_status(f"Test preview generated: H1-H{max_heading}")
        except Exception as exc:
            logger.exception("Test preview generation failed")
            messagebox.showerror("Test preview error", str(exc))
            self.set_status("Test preview generation failed")

    def _page_size(self) -> str:
        return self.page_size_getter().strip() or "A4"

    def _resolve_output_dir(self, markdown_path: Path | None) -> Path:
        configured = self.configured_output_dir_getter().strip()
        if configured:
            return Path(configured)
        if markdown_path is not None:
            return default_output_dir(markdown_path)
        return default_output_dir(None)

    @staticmethod
    def _detect_markdown_features(markdown_text: str) -> dict[str, bool]:
        return {
            "blockquote": bool(re.search(r"(?m)^\s{0,3}>\s+", markdown_text)),
            "unordered_list": bool(re.search(r"(?m)^\s*[-*+]\s+", markdown_text)),
            "ordered_list": bool(re.search(r"(?m)^\s*\d+\.\s+", markdown_text)),
            "table": "|" in markdown_text
            and bool(re.search(r"(?m)^\s*\|?.*\|.*$", markdown_text)),
            "code_fence": bool(re.search(r"(?m)^\s*(```|~~~)", markdown_text)),
            "hr": bool(re.search(r"(?m)^\s{0,3}([-*_])\s*(\1\s*){2,}$", markdown_text)),
            "footnote": "[^" in markdown_text,
            "image": "![" in markdown_text,
            "link": bool(re.search(r"\[[^\]]+\]\([^\)]+\)", markdown_text)),
        }

    def _build_generated_test_markdown(self, source_markdown: str) -> str:
        max_heading = max(1, min(6, self.heading_level_detector(source_markdown)))
        features = self._detect_markdown_features(source_markdown)
        lines: list[str] = []

        for level in range(1, max_heading + 1):
            lines.append(f"{'#' * level} Heading level {level}")
            lines.append("")

        lines.append("Demo paragraph with `inline code`.")
        if features["link"]:
            lines[-1] += " Example link: [Documentation](https://example.com)."
        lines.append("")

        if features["blockquote"]:
            lines.extend(["> Blockquote example", ""])
        if features["unordered_list"]:
            lines.extend(["- Item A", "- Item B", ""])
        if features["ordered_list"]:
            lines.extend(["1. Step 1", "2. Step 2", ""])
        if features["table"]:
            lines.extend(
                [
                    "| Column A | Column B |",
                    "|---|---|",
                    "| Value 1 | Value 2 |",
                    "",
                ]
            )
        if features["code_fence"]:
            lines.extend(
                [
                    "```python",
                    "def greet(name: str) -> str:",
                    '    return f"Hello {name}"',
                    "",
                    'print(greet("world"))',
                    "```",
                    "",
                ]
            )
        if features["hr"]:
            lines.extend(["---", ""])
        if features["image"]:
            lines.extend(["![Example image](image.png)", ""])
        if features["footnote"]:
            lines.extend(
                [
                    "Text with footnote[^n1].",
                    "",
                    "[^n1]: Auto-generated footnote example.",
                    "",
                ]
            )

        return "\n".join(lines).strip() or _SANDBOX_MARKDOWN

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
        output_dir = (
            Path(configured_output_dir)
            if configured_output_dir
            else default_output_dir(md_path)
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
