"""Main GUI application for Nectar Render."""

from __future__ import annotations

import logging
import re
import time
import tkinter as tk
import webbrowser
from pathlib import Path
from tkinter import filedialog, font as tkfont, messagebox, ttk

from ..config import (
    AppConfig,
    CompressionOptions,
    ExportOptions,
    PDF_COMPRESSION_PROFILES,
    StyleOptions,
    OUTPUT_FORMATS,
    UI_THEMES,
    DEFAULT_BODY_FONT,
    DEFAULT_CODE_FONT,
    DEFAULT_HEADING_COLOR,
    FALLBACK_FONTS,
)
from ..converter.exporter import build_html_from_markdown
from ..converter.highlight import list_available_styles
from ..services.conversion_service import ConversionService
from ..utils.paths import application_data_dir, default_output_dir
from .state_manager import StateManager, _safe_int, _safe_float
from .theme import apply_ui_theme
from .widgets import FontAutocomplete

logger = logging.getLogger(__name__)

PREVIEW_DEBOUNCE_MS = 300

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
    "[^note]: {{FOOTNOTE_NOTE}}\n"
)

_PRESETS_FILENAME = "presets.json"
_STATE_FILENAME = "last_state.json"

# Ordered list of Tk variable names for heading levels
_HEADING_COLOR_KEYS = [f"heading_h{lvl}_color_var" for lvl in range(1, 7)]
_HEADING_SIZE_KEYS = [f"heading_h{lvl}_size_var" for lvl in range(1, 7)]


class NectarRenderApp:
    """Tkinter GUI for Nectar Render."""

    def __init__(self, root: tk.Tk) -> None:
        self.root = root
        self.root.title("Nectar Render")
        self.root.geometry("1120x820")

        self.config = AppConfig()
        self.service = ConversionService()
        self.preview_job: str | None = None
        self.live_preview_path: Path | None = None

        app_dir = application_data_dir()
        app_dir.mkdir(parents=True, exist_ok=True)

        self.state_mgr = StateManager(
            root=root,
            app_dir=app_dir,
            presets_filename=_PRESETS_FILENAME,
            state_filename=_STATE_FILENAME,
        )

        self._create_tk_vars()

        self.heading_level_rows: dict[int, tuple[tk.Widget, ...]] = {}
        self.detected_heading_max_level = 3
        self.preset_names: list[str] = []
        self.available_fonts = self._get_system_fonts()

        # Register Tk variables with state manager
        self.state_mgr.set_tk_vars(
            tk_vars=self._tk_var_dict(),
            status_var=self.status_var,
            ensure_font_option=self._ensure_font_option,
        )

        apply_ui_theme(self.root, self.ui_theme_var.get())
        self._build_ui()
        self.markdown_var.trace_add("write", self._on_markdown_path_changed)
        self._setup_font_controls()
        self._refresh_preset_list()
        self.state_mgr.load_last_state()
        self._refresh_heading_controls_from_markdown()
        self._bind_shortcuts()
        self._setup_live_preview_traces()
        self.state_mgr.initialize_history()

    # --- Tk variables ---------------------------------------------------------

    def _create_tk_vars(self) -> None:
        cfg = self.config

        self.markdown_var = tk.StringVar(value="")
        self.output_dir_var = tk.StringVar(value=str(default_output_dir(None)))
        self.format_var = tk.StringVar(value="PDF")
        self.pdf_compression_enabled_var = tk.BooleanVar(value=cfg.export.compression.enabled)
        self.pdf_compression_profile_var = tk.StringVar(value=cfg.export.compression.profile)
        self.pdf_remove_metadata_var = tk.BooleanVar(value=cfg.export.compression.remove_metadata)
        self.body_font_var = tk.StringVar(value=cfg.style.body_font)
        self.heading_font_var = tk.StringVar(value=cfg.style.heading_font)
        self.heading_color_var = tk.StringVar(value=cfg.style.heading_color)

        # Per-level heading variables (built via loop)
        self.heading_color_vars: dict[int, tk.StringVar] = {}
        self.heading_size_vars: dict[int, tk.IntVar] = {}
        for level in range(1, 7):
            self.heading_color_vars[level] = tk.StringVar(
                value=getattr(cfg.style, f"heading_h{level}_color")
            )
            self.heading_size_vars[level] = tk.IntVar(
                value=getattr(cfg.style, f"heading_h{level}_size_px")
            )

        self.code_font_var = tk.StringVar(value=cfg.style.code_font)
        self.body_size_var = tk.IntVar(value=cfg.style.body_font_size)
        self.code_size_var = tk.IntVar(value=cfg.style.code_font_size)
        self.code_line_height_var = tk.DoubleVar(value=cfg.style.code_line_height)
        self.line_height_var = tk.DoubleVar(value=cfg.style.line_height)
        self.code_theme_var = tk.StringVar(value=cfg.style.code_theme)
        self.margin_top_var = tk.DoubleVar(value=cfg.style.margin_top_mm)
        self.margin_right_var = tk.DoubleVar(value=cfg.style.margin_right_mm)
        self.margin_bottom_var = tk.DoubleVar(value=cfg.style.margin_bottom_mm)
        self.margin_left_var = tk.DoubleVar(value=cfg.style.margin_left_mm)
        self.footer_text_var = tk.StringVar(value=cfg.style.footer_text)
        footer_align_value = (cfg.style.footer_align or "right").lower()
        self.footer_align_var = tk.StringVar(value="Center" if footer_align_value == "center" else "Right")
        self.include_notes_var = tk.BooleanVar(value=True)
        self.footnote_size_var = tk.DoubleVar(value=cfg.style.footnote_font_size)
        self.footnote_text_color_var = tk.StringVar(value=cfg.style.footnote_text_color)
        self.footnote_marker_color_var = tk.StringVar(value=cfg.style.footnote_marker_color)
        self.table_stripes_var = tk.BooleanVar(value=cfg.style.table_row_stripes)
        self.table_odd_color_var = tk.StringVar(value=cfg.style.table_row_odd_color)
        self.table_even_color_var = tk.StringVar(value=cfg.style.table_row_even_color)
        self.table_pad_y_var = tk.IntVar(value=cfg.style.table_cell_padding_y_px)
        self.table_pad_x_var = tk.IntVar(value=cfg.style.table_cell_padding_x_px)
        self.image_scale_var = tk.DoubleVar(value=round(cfg.style.image_scale * 100.0, 1))
        self.show_horizontal_rules_var = tk.BooleanVar(value=cfg.style.show_horizontal_rules)
        self.preset_var = tk.StringVar(value="")
        self.ui_theme_var = tk.StringVar(value="Light")
        self.auto_preview_var = tk.BooleanVar(value=True)
        self.status_var = tk.StringVar(value="Ready")

    def _tk_var_dict(self) -> dict[str, tk.Variable]:
        """Return a name->Variable mapping for all state-tracked variables."""
        d: dict[str, tk.Variable] = {
            "markdown_var": self.markdown_var,
            "output_dir_var": self.output_dir_var,
            "format_var": self.format_var,
            "pdf_compression_enabled_var": self.pdf_compression_enabled_var,
            "pdf_compression_profile_var": self.pdf_compression_profile_var,
            "pdf_remove_metadata_var": self.pdf_remove_metadata_var,
            "body_font_var": self.body_font_var,
            "heading_font_var": self.heading_font_var,
            "heading_color_var": self.heading_color_var,
            "code_font_var": self.code_font_var,
            "body_size_var": self.body_size_var,
            "code_size_var": self.code_size_var,
            "code_line_height_var": self.code_line_height_var,
            "line_height_var": self.line_height_var,
            "code_theme_var": self.code_theme_var,
            "margin_top_var": self.margin_top_var,
            "margin_right_var": self.margin_right_var,
            "margin_bottom_var": self.margin_bottom_var,
            "margin_left_var": self.margin_left_var,
            "footer_text_var": self.footer_text_var,
            "footer_align_var": self.footer_align_var,
            "include_notes_var": self.include_notes_var,
            "footnote_size_var": self.footnote_size_var,
            "footnote_text_color_var": self.footnote_text_color_var,
            "footnote_marker_color_var": self.footnote_marker_color_var,
            "table_stripes_var": self.table_stripes_var,
            "table_odd_color_var": self.table_odd_color_var,
            "table_even_color_var": self.table_even_color_var,
            "table_pad_y_var": self.table_pad_y_var,
            "table_pad_x_var": self.table_pad_x_var,
            "image_scale_var": self.image_scale_var,
            "show_horizontal_rules_var": self.show_horizontal_rules_var,
            "ui_theme_var": self.ui_theme_var,
            "auto_preview_var": self.auto_preview_var,
        }
        for level in range(1, 7):
            d[f"heading_h{level}_color_var"] = self.heading_color_vars[level]
            d[f"heading_h{level}_size_var"] = self.heading_size_vars[level]
        return d

    # --- UI construction ------------------------------------------------------

    def _build_ui(self) -> None:
        container = ttk.Frame(self.root, padding=14)
        container.pack(fill=tk.BOTH, expand=True)
        container.columnconfigure(0, weight=1)
        container.rowconfigure(1, weight=1)

        # Header
        header = ttk.Frame(container, style="Card.TFrame", padding=(12, 10))
        header.grid(row=0, column=0, sticky="ew", pady=(0, 10))
        header.columnconfigure(1, weight=1)

        ttk.Label(header, text="Nectar Render", style="Title.TLabel").grid(row=0, column=0, sticky="w")
        ttk.Label(header, text="Markdown to PDF / HTML desktop converter", style="Muted.TLabel").grid(row=1, column=0, sticky="w", pady=(2, 0))

        header_actions = ttk.Frame(header)
        header_actions.grid(row=0, column=1, rowspan=2, sticky="e")
        ttk.Button(header_actions, text="Preview HTML", command=self._preview_html).pack(side=tk.LEFT)
        ttk.Button(header_actions, text="Test preview", command=self._preview_test_html).pack(side=tk.LEFT, padx=(8, 0))
        ttk.Button(header_actions, text="Convert", command=self._convert, style="Primary.TButton").pack(side=tk.LEFT, padx=(8, 0))

        # Scrollable panel
        left_shell = ttk.Frame(container, padding=2)
        left_shell.grid(row=1, column=0, sticky="nsew")

        self.left_canvas = tk.Canvas(left_shell, highlightthickness=0, borderwidth=0, bg=self.root.cget("bg"))
        left_scrollbar = ttk.Scrollbar(left_shell, orient=tk.VERTICAL, command=self.left_canvas.yview)
        self.left_scrollbar = left_scrollbar
        self.left_scroll_enabled = False
        self.left_canvas.configure(yscrollcommand=left_scrollbar.set)
        left_scrollbar.pack(side=tk.RIGHT, fill=tk.Y)
        self.left_canvas.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)

        left_panel = ttk.Frame(self.left_canvas)
        left_panel.columnconfigure(0, weight=1)
        self.left_canvas_window = self.left_canvas.create_window((0, 0), window=left_panel, anchor="nw")

        def _update_left_scroll_state() -> None:
            bbox = self.left_canvas.bbox("all")
            if not bbox:
                self.left_scroll_enabled = False
                return
            content_height = max(0, bbox[3] - bbox[1])
            canvas_height = max(1, self.left_canvas.winfo_height())
            self.left_scroll_enabled = content_height > (canvas_height + 2)
            if not self.left_scroll_enabled:
                self.left_canvas.yview_moveto(0.0)
                try:
                    self.left_scrollbar.state(["disabled"])
                except tk.TclError:
                    pass
            else:
                try:
                    self.left_scrollbar.state(["!disabled"])
                except tk.TclError:
                    pass

        def _sync_left_scrollregion(_event: tk.Event[tk.Misc]) -> None:
            self.left_canvas.configure(scrollregion=self.left_canvas.bbox("all"))
            _update_left_scroll_state()

        def _sync_left_width(event: tk.Event[tk.Misc]) -> None:
            self.left_canvas.itemconfigure(self.left_canvas_window, width=event.width)
            self.root.after_idle(_update_left_scroll_state)

        def _on_left_mousewheel(event: tk.Event[tk.Misc]) -> str:
            if not self.left_scroll_enabled:
                return ""
            try:
                x = event.x_root
                y = event.y_root
                cx = self.left_canvas.winfo_rootx()
                cy = self.left_canvas.winfo_rooty()
                cw = self.left_canvas.winfo_width()
                ch = self.left_canvas.winfo_height()
                if not (cx <= x <= cx + cw and cy <= y <= cy + ch):
                    return ""
            except tk.TclError:
                return ""

            delta = 0
            if hasattr(event, "delta") and event.delta:
                delta = -1 if event.delta > 0 else 1
            elif getattr(event, "num", None) == 4:
                delta = -1
            elif getattr(event, "num", None) == 5:
                delta = 1
            if delta:
                self.left_canvas.yview_scroll(delta, "units")
            return "break"

        left_panel.bind("<Configure>", _sync_left_scrollregion)
        self.left_canvas.bind("<Configure>", _sync_left_width)
        self.root.bind_all("<MouseWheel>", _on_left_mousewheel, add="+")
        self.root.bind_all("<Button-4>", _on_left_mousewheel, add="+")
        self.root.bind_all("<Button-5>", _on_left_mousewheel, add="+")

        # --- Basic settings ---------------------------------------------------
        basic_box = ttk.LabelFrame(left_panel, text="Basic settings", padding=8)
        basic_box.pack(fill=tk.X, pady=(0, 6))
        basic_box.columnconfigure(1, weight=1)

        ttk.Label(basic_box, text="Markdown file (.md)").grid(row=0, column=0, sticky="w")
        self.markdown_entry = ttk.Entry(basic_box, textvariable=self.markdown_var)
        self.markdown_entry.grid(row=0, column=1, columnspan=4, sticky="ew", padx=(8, 8))
        ttk.Button(basic_box, text="Browse", command=self._select_markdown_file).grid(row=0, column=5)

        ttk.Label(basic_box, text="Output folder").grid(row=1, column=0, sticky="w", pady=(6, 0))
        self.output_dir_entry = ttk.Entry(basic_box, textvariable=self.output_dir_var)
        self.output_dir_entry.grid(row=1, column=1, columnspan=4, sticky="ew", padx=(8, 8), pady=(6, 0))
        ttk.Button(basic_box, text="Choose", command=self._select_output_directory).grid(row=1, column=5, pady=(6, 0))

        options_row = ttk.Frame(basic_box)
        options_row.grid(row=2, column=0, columnspan=6, sticky="ew", pady=(8, 0))

        ttk.Label(options_row, text="Format").pack(side=tk.LEFT)
        ttk.Combobox(
            options_row, textvariable=self.format_var,
            values=OUTPUT_FORMATS, state="readonly", width=10,
        ).pack(side=tk.LEFT, padx=(8, 16))

        ttk.Label(options_row, text="UI Theme").pack(side=tk.LEFT)
        ttk.Combobox(
            options_row, textvariable=self.ui_theme_var,
            values=UI_THEMES, state="readonly", width=10,
        ).pack(side=tk.LEFT, padx=(8, 8))
        ttk.Button(options_row, text="Apply", command=self._apply_theme).pack(side=tk.LEFT, padx=(0, 16))

        ttk.Label(options_row, text="Preset").pack(side=tk.LEFT)
        self.preset_combo = ttk.Combobox(options_row, textvariable=self.preset_var, values=[], width=24)
        self.preset_combo.pack(side=tk.LEFT, padx=(8, 8))
        ttk.Button(options_row, text="Save", command=self._save_preset).pack(side=tk.LEFT)
        ttk.Button(options_row, text="Load", command=self._load_selected_preset).pack(side=tk.LEFT, padx=(8, 0))

        compression_row = ttk.Frame(basic_box)
        compression_row.grid(row=3, column=0, columnspan=6, sticky="ew", pady=(8, 0))
        ttk.Checkbutton(
            compression_row, text="Automatic PDF compression",
            variable=self.pdf_compression_enabled_var,
        ).pack(side=tk.LEFT)
        ttk.Label(compression_row, text="Profile").pack(side=tk.LEFT, padx=(12, 0))
        ttk.Combobox(
            compression_row, textvariable=self.pdf_compression_profile_var,
            values=PDF_COMPRESSION_PROFILES, state="readonly", width=10,
        ).pack(side=tk.LEFT, padx=(8, 8))
        ttk.Checkbutton(
            compression_row, text="Remove metadata",
            variable=self.pdf_remove_metadata_var,
        ).pack(side=tk.LEFT, padx=(8, 0))

        # --- Typography -------------------------------------------------------
        typography_box = ttk.LabelFrame(left_panel, text="Typography", padding=10)
        typography_box.pack(fill=tk.X, pady=(0, 8))

        ttk.Label(typography_box, text="Body font").grid(row=0, column=0, sticky="w")
        body_font_cell = ttk.Frame(typography_box)
        body_font_cell.grid(row=0, column=1, sticky="w", padx=(8, 8))
        self.body_font_combo = FontAutocomplete(
            root=self.root, parent=body_font_cell,
            textvariable=self.body_font_var, values=self.available_fonts,
            width=22, on_commit=lambda _v: self._update_font_previews(),
        )
        self.body_font_combo.frame.pack(side=tk.LEFT)
        self.body_font_preview = ttk.Label(body_font_cell, text="AaBb 123", width=10)
        self.body_font_preview.pack(side=tk.LEFT, padx=(8, 0))

        ttk.Label(typography_box, text="Size").grid(row=0, column=2, sticky="w")
        ttk.Spinbox(typography_box, from_=8, to=24, textvariable=self.body_size_var, width=7).grid(row=0, column=3, sticky="w", padx=(8, 8))
        ttk.Label(typography_box, text="Line height").grid(row=0, column=4, sticky="w")
        ttk.Spinbox(typography_box, from_=1.0, to=2.4, increment=0.1, textvariable=self.line_height_var, width=7).grid(row=0, column=5, sticky="w", padx=(8, 0))

        ttk.Label(typography_box, text="Heading font").grid(row=1, column=0, sticky="w", pady=(8, 0))
        heading_font_cell = ttk.Frame(typography_box)
        heading_font_cell.grid(row=1, column=1, sticky="w", padx=(8, 8), pady=(8, 0))
        self.heading_font_combo = FontAutocomplete(
            root=self.root, parent=heading_font_cell,
            textvariable=self.heading_font_var, values=self.available_fonts,
            width=22, on_commit=lambda _v: self._update_font_previews(),
        )
        self.heading_font_combo.frame.pack(side=tk.LEFT)
        self.heading_font_preview = ttk.Label(heading_font_cell, text="AaBb 123", width=10)
        self.heading_font_preview.pack(side=tk.LEFT, padx=(8, 0))

        # --- Page margins -----------------------------------------------------
        margin_box = ttk.LabelFrame(left_panel, text="Page margins", padding=10)
        margin_box.pack(fill=tk.X, pady=(0, 8))
        ttk.Label(margin_box, text="Top").grid(row=0, column=0, sticky="w")
        ttk.Spinbox(margin_box, from_=5, to=50, increment=0.5, textvariable=self.margin_top_var, width=7).grid(row=0, column=1, sticky="w", padx=(8, 12))
        ttk.Label(margin_box, text="Right").grid(row=0, column=2, sticky="w")
        ttk.Spinbox(margin_box, from_=5, to=50, increment=0.5, textvariable=self.margin_right_var, width=7).grid(row=0, column=3, sticky="w", padx=(8, 12))
        ttk.Label(margin_box, text="Bottom").grid(row=1, column=0, sticky="w", pady=(8, 0))
        ttk.Spinbox(margin_box, from_=5, to=50, increment=0.5, textvariable=self.margin_bottom_var, width=7).grid(row=1, column=1, sticky="w", padx=(8, 12), pady=(8, 0))
        ttk.Label(margin_box, text="Left").grid(row=1, column=2, sticky="w", pady=(8, 0))
        ttk.Spinbox(margin_box, from_=5, to=50, increment=0.5, textvariable=self.margin_left_var, width=7).grid(row=1, column=3, sticky="w", padx=(8, 12), pady=(8, 0))

        # --- Advanced panel (two columns) ------------------------------------
        self.advanced_panel = ttk.Frame(left_panel)
        self.advanced_panel.pack(fill=tk.BOTH, expand=True, pady=(0, 8))
        self.advanced_panel.columnconfigure(0, weight=1)
        self.advanced_panel.columnconfigure(1, weight=1)

        # Advanced typography
        typography_advanced = ttk.LabelFrame(self.advanced_panel, text="Advanced typography", padding=10)
        typography_advanced.grid(row=0, column=0, sticky="nsew", pady=(0, 8), padx=(0, 6))
        typography_advanced.columnconfigure(1, weight=1)

        ttk.Label(typography_advanced, text="Global heading color").grid(row=0, column=0, sticky="w")
        ttk.Entry(typography_advanced, textvariable=self.heading_color_var, width=10).grid(row=0, column=1, sticky="w", padx=(8, 12))
        self.heading_detected_label = ttk.Label(typography_advanced, text="", style="Muted.TLabel")
        self.heading_detected_label.grid(row=0, column=2, columnspan=2, sticky="w", padx=(8, 0))

        ttk.Label(typography_advanced, text="Level").grid(row=1, column=0, sticky="w", pady=(8, 0))
        ttk.Label(typography_advanced, text="Font").grid(row=1, column=1, sticky="w", pady=(8, 0))
        ttk.Label(typography_advanced, text="Size").grid(row=1, column=2, sticky="w", pady=(8, 0))
        ttk.Label(typography_advanced, text="Color").grid(row=1, column=3, sticky="w", pady=(8, 0))

        for level in range(1, 7):
            row = level + 1
            level_label = ttk.Label(typography_advanced, text=f"H{level}")
            level_label.grid(row=row, column=0, sticky="w", pady=(8, 0))
            level_font_label = ttk.Label(typography_advanced, textvariable=self.heading_font_var, width=24)
            level_font_label.grid(row=row, column=1, sticky="w", pady=(8, 0))
            level_size_spin = ttk.Spinbox(typography_advanced, from_=10, to=96, textvariable=self.heading_size_vars[level], width=7)
            level_size_spin.grid(row=row, column=2, sticky="w", padx=(8, 12), pady=(8, 0))
            level_color_entry = ttk.Entry(typography_advanced, textvariable=self.heading_color_vars[level], width=10)
            level_color_entry.grid(row=row, column=3, sticky="w", padx=(0, 8), pady=(8, 0))
            self.heading_level_rows[level] = (level_label, level_font_label, level_size_spin, level_color_entry)

        # Code styling
        code_box = ttk.LabelFrame(self.advanced_panel, text="Code", padding=10)
        code_box.grid(row=0, column=1, sticky="nsew", pady=(0, 8), padx=(6, 0))
        ttk.Label(code_box, text="Code theme").grid(row=0, column=0, sticky="w")
        code_theme_cell = ttk.Frame(code_box)
        code_theme_cell.grid(row=0, column=1, sticky="w", padx=(8, 12))
        self.code_theme_combo = FontAutocomplete(
            root=self.root, parent=code_theme_cell,
            textvariable=self.code_theme_var,
            values=list_available_styles(), width=18,
            allow_custom_values=False,
        )
        self.code_theme_combo.frame.pack(side=tk.LEFT)

        ttk.Label(code_box, text="Code font").grid(row=1, column=0, sticky="w", pady=(8, 0))
        code_font_cell = ttk.Frame(code_box)
        code_font_cell.grid(row=1, column=1, sticky="ew", padx=(8, 12), pady=(8, 0))
        self.code_font_combo = FontAutocomplete(
            root=self.root, parent=code_font_cell,
            textvariable=self.code_font_var, values=self.available_fonts,
            width=22, on_commit=lambda _v: self._update_font_previews(),
        )
        self.code_font_combo.frame.pack(side=tk.LEFT)
        self.code_font_preview = ttk.Label(code_font_cell, text="AaBb 123", width=10)
        self.code_font_preview.pack(side=tk.LEFT, padx=(8, 0))
        ttk.Label(code_box, text="Size").grid(row=1, column=2, sticky="w", pady=(8, 0))
        ttk.Spinbox(code_box, from_=8, to=24, textvariable=self.code_size_var, width=7).grid(row=1, column=3, sticky="w", padx=(8, 0), pady=(8, 0))
        ttk.Label(code_box, text="Line height").grid(row=1, column=4, sticky="w", padx=(14, 0), pady=(8, 0))
        ttk.Spinbox(
            code_box, from_=1.0, to=2.4, increment=0.1,
            textvariable=self.code_line_height_var, width=7,
        ).grid(row=1, column=5, sticky="w", padx=(8, 0), pady=(8, 0))

        # Tables & separators
        table_box = ttk.LabelFrame(self.advanced_panel, text="Tables & separators", padding=10)
        table_box.grid(row=1, column=0, sticky="nsew", pady=(0, 8), padx=(0, 6))
        ttk.Checkbutton(
            table_box, text="Alternating row colors (striping)",
            variable=self.table_stripes_var,
        ).grid(row=0, column=0, columnspan=4, sticky="w")
        ttk.Label(table_box, text="Odd row color").grid(row=1, column=0, sticky="w", pady=(8, 0))
        ttk.Entry(table_box, textvariable=self.table_odd_color_var, width=10).grid(row=1, column=1, sticky="w", padx=(8, 12), pady=(8, 0))
        ttk.Label(table_box, text="Even row color").grid(row=1, column=2, sticky="w", pady=(8, 0))
        ttk.Entry(table_box, textvariable=self.table_even_color_var, width=10).grid(row=1, column=3, sticky="w", padx=(8, 0), pady=(8, 0))
        ttk.Label(table_box, text="Padding Y (px)").grid(row=2, column=0, sticky="w", pady=(8, 0))
        ttk.Spinbox(table_box, from_=0, to=30, textvariable=self.table_pad_y_var, width=7).grid(row=2, column=1, sticky="w", padx=(8, 12), pady=(8, 0))
        ttk.Label(table_box, text="Padding X (px)").grid(row=2, column=2, sticky="w", pady=(8, 0))
        ttk.Spinbox(table_box, from_=0, to=40, textvariable=self.table_pad_x_var, width=7).grid(row=2, column=3, sticky="w", padx=(8, 0), pady=(8, 0))
        ttk.Label(table_box, text="Image size (%)").grid(row=3, column=0, sticky="w", pady=(8, 0))
        ttk.Scale(table_box, from_=40, to=100, variable=self.image_scale_var, orient=tk.HORIZONTAL).grid(row=3, column=1, columnspan=2, sticky="ew", padx=(8, 12), pady=(8, 0))
        image_scale_input = ttk.Frame(table_box)
        image_scale_input.grid(row=3, column=3, sticky="w", pady=(8, 0))
        ttk.Spinbox(
            image_scale_input, from_=40, to=100, increment=0.1,
            textvariable=self.image_scale_var, width=7,
        ).pack(side=tk.LEFT)
        ttk.Label(image_scale_input, text="%", width=2).pack(side=tk.LEFT, padx=(4, 0))
        ttk.Checkbutton(
            table_box, text="Show horizontal rules (---)",
            variable=self.show_horizontal_rules_var,
        ).grid(row=4, column=0, columnspan=4, sticky="w", pady=(8, 0))

        # Footnotes & footer
        footnote_box = ttk.LabelFrame(self.advanced_panel, text="Footnotes", padding=10)
        footnote_box.grid(row=1, column=1, sticky="nsew", pady=(0, 8), padx=(6, 0))
        ttk.Checkbutton(footnote_box, text="Enable footnotes", variable=self.include_notes_var).grid(row=0, column=0, columnspan=4, sticky="w")
        ttk.Label(footnote_box, text="Size").grid(row=1, column=0, sticky="w", pady=(8, 0))
        ttk.Spinbox(footnote_box, from_=7, to=16, increment=0.5, textvariable=self.footnote_size_var, width=7).grid(row=1, column=1, sticky="w", padx=(8, 12), pady=(8, 0))
        ttk.Label(footnote_box, text="Text color").grid(row=1, column=2, sticky="w", pady=(8, 0))
        ttk.Entry(footnote_box, textvariable=self.footnote_text_color_var, width=10).grid(row=1, column=3, sticky="w", padx=(8, 0), pady=(8, 0))
        ttk.Label(footnote_box, text="Marker color").grid(row=2, column=0, sticky="w", pady=(8, 0))
        ttk.Entry(footnote_box, textvariable=self.footnote_marker_color_var, width=10).grid(row=2, column=1, sticky="w", padx=(8, 12), pady=(8, 0))

        ttk.Separator(footnote_box, orient=tk.HORIZONTAL).grid(row=3, column=0, columnspan=4, sticky="ew", pady=(10, 8))
        ttk.Label(footnote_box, text="PDF pagination").grid(row=4, column=0, sticky="w")
        ttk.Label(footnote_box, text="Alignment").grid(row=4, column=1, sticky="w", padx=(8, 0))
        ttk.Combobox(
            footnote_box, textvariable=self.footer_align_var,
            values=("Right", "Center"), state="readonly", width=10,
        ).grid(row=4, column=2, sticky="w")
        ttk.Label(footnote_box, text="Footer text").grid(row=5, column=0, sticky="w", pady=(8, 0))
        ttk.Entry(footnote_box, textvariable=self.footer_text_var, width=30).grid(row=5, column=1, columnspan=3, sticky="ew", padx=(8, 0), pady=(8, 0))

        # Status bar
        ttk.Label(container, textvariable=self.status_var).grid(row=2, column=0, sticky="w", pady=(8, 0))

        help_text = (
            "Page breaks: <!-- pagebreak -->, \\pagebreak, [[PAGEBREAK]]\n"
            "All settings are visible. Use scrollbars for long paths."
        )
        ttk.Label(container, text=help_text, justify=tk.LEFT, style="Muted.TLabel").grid(row=3, column=0, sticky="w", pady=(6, 0))

    # --- live preview traces --------------------------------------------------

    def _setup_live_preview_traces(self) -> None:
        all_vars = list(self._tk_var_dict().values())
        for var in all_vars:
            var.trace_add("write", self._on_option_changed)

    def _bind_shortcuts(self) -> None:
        self.root.bind_all("<Control-z>", self.state_mgr.undo_action)
        self.root.bind_all("<Control-Alt-z>", self.state_mgr.redo_action)

    # --- font controls --------------------------------------------------------

    def _setup_font_controls(self) -> None:
        self.body_font_var.trace_add("write", lambda *_: self._update_font_previews())
        self.heading_font_var.trace_add("write", lambda *_: self._update_font_previews())
        self.code_font_var.trace_add("write", lambda *_: self._update_font_previews())
        self._update_font_previews()

    def _update_font_previews(self) -> None:
        self._set_preview_font(self.body_font_preview, self.body_font_var.get())
        self._set_preview_font(self.heading_font_preview, self.heading_font_var.get())
        self._set_preview_font(self.code_font_preview, self.code_font_var.get())

    def _set_preview_font(self, label: ttk.Label, family: str) -> None:
        cleaned = (family or "").strip() or DEFAULT_BODY_FONT
        try:
            label.configure(font=(cleaned, 10))
        except tk.TclError:
            label.configure(font=(DEFAULT_BODY_FONT, 10))

    def _get_system_fonts(self) -> list[str]:
        try:
            families = tkfont.families(self.root)
        except tk.TclError:
            families = ()
        cleaned = {str(name).strip() for name in families if str(name).strip()}
        if not cleaned:
            cleaned = set(FALLBACK_FONTS)
        else:
            cleaned.update(FALLBACK_FONTS)
        return sorted(cleaned, key=str.casefold)

    def _refresh_font_combobox_values(self) -> None:
        values = list(self.available_fonts)
        self.body_font_combo.set_values(values)
        self.heading_font_combo.set_values(values)
        self.code_font_combo.set_values(values)

    def _ensure_font_option(self, font_name: str) -> None:
        normalized = font_name.strip()
        if not normalized:
            return
        if normalized not in self.available_fonts:
            self.available_fonts.append(normalized)
            self.available_fonts.sort(key=str.casefold)
            if hasattr(self, "body_font_combo"):
                self._refresh_font_combobox_values()

    # --- option change handler ------------------------------------------------

    def _on_option_changed(self, *_args: object) -> None:
        self.state_mgr.record_history_state()
        self.state_mgr.schedule_persist_state()

    # --- heading level detection ----------------------------------------------

    def _detect_markdown_max_heading_level(self, markdown_file: Path) -> int:
        try:
            text = markdown_file.read_text(encoding="utf-8")
        except OSError:
            return 3
        return self._detect_markdown_max_heading_level_from_text(text)

    def _refresh_heading_controls_from_markdown(self) -> None:
        max_level = 3
        raw_path = self.markdown_var.get().strip()
        if raw_path:
            candidate = Path(raw_path)
            if candidate.exists() and candidate.is_file() and candidate.suffix.lower() == ".md":
                max_level = self._detect_markdown_max_heading_level(candidate)

        self.detected_heading_max_level = max(1, min(6, max_level))
        for level, widgets in self.heading_level_rows.items():
            if level <= self.detected_heading_max_level:
                for widget in widgets:
                    widget.grid()
            else:
                for widget in widgets:
                    widget.grid_remove()

        self.heading_detected_label.configure(text=f"Detected headings: H1 to H{self.detected_heading_max_level}")

    def _on_markdown_path_changed(self, *_args: object) -> None:
        self._refresh_heading_controls_from_markdown()

    # --- presets (delegated to state_mgr) ------------------------------------

    def _refresh_preset_list(self) -> None:
        self.preset_names = self.state_mgr.load_presets()
        self.preset_combo["values"] = self.preset_names

    def _save_preset(self) -> None:
        name = self.preset_var.get().strip()
        self.state_mgr.save_preset(name, self.status_var)
        self._refresh_preset_list()

    def _load_selected_preset(self) -> None:
        name = self.preset_var.get().strip()
        self.state_mgr.load_selected_preset(name, self.status_var)

    # --- file selection -------------------------------------------------------

    def _select_markdown_file(self) -> None:
        path = filedialog.askopenfilename(filetypes=[("Markdown", "*.md"), ("All files", "*.*")])
        if not path:
            return
        self.markdown_var.set(path)
        out_dir = default_output_dir(Path(path))
        self.output_dir_var.set(str(out_dir))

    def _select_output_directory(self) -> None:
        path = filedialog.askdirectory()
        if path:
            self.output_dir_var.set(path)

    # --- collect style/export options -----------------------------------------

    def _collect_style(self) -> StyleOptions:
        body_font_size = _safe_int(self.body_size_var.get(), 12)
        code_font_size = _safe_int(self.code_size_var.get(), 11)
        code_line_height = _safe_float(self.code_line_height_var.get(), self.config.style.code_line_height)
        line_height = _safe_float(self.line_height_var.get(), 1.5)
        margin_top = _safe_float(self.margin_top_var.get(), 25.4)
        margin_right = _safe_float(self.margin_right_var.get(), 25.4)
        margin_bottom = _safe_float(self.margin_bottom_var.get(), 25.4)
        margin_left = _safe_float(self.margin_left_var.get(), 25.4)
        footer_align = "center" if self.footer_align_var.get().strip().lower() == "center" else "right"
        footnote_size = _safe_float(self.footnote_size_var.get(), 9.0)
        table_pad_y = _safe_int(self.table_pad_y_var.get(), self.config.style.table_cell_padding_y_px)
        table_pad_x = _safe_int(self.table_pad_x_var.get(), self.config.style.table_cell_padding_x_px)
        image_scale_pct = _safe_float(self.image_scale_var.get(), self.config.style.image_scale * 100.0)
        image_scale = max(0.4, min(1.0, image_scale_pct / 100.0))

        kwargs: dict[str, object] = {
            "body_font": self.body_font_var.get().strip() or DEFAULT_BODY_FONT,
            "body_font_size": body_font_size,
            "line_height": line_height,
            "heading_font": self.heading_font_var.get().strip() or DEFAULT_BODY_FONT,
            "heading_color": self.heading_color_var.get().strip() or DEFAULT_HEADING_COLOR,
            "code_font": self.code_font_var.get().strip() or DEFAULT_CODE_FONT,
            "code_font_size": code_font_size,
            "code_line_height": code_line_height,
            "code_theme": self.code_theme_var.get().strip() or "default",
            "margin_top_mm": margin_top,
            "margin_right_mm": margin_right,
            "margin_bottom_mm": margin_bottom,
            "margin_left_mm": margin_left,
            "footer_text": self.footer_text_var.get().strip(),
            "footer_align": footer_align,
            "include_footnotes": bool(self.include_notes_var.get()),
            "footnote_font_size": footnote_size,
            "footnote_text_color": self.footnote_text_color_var.get().strip() or "#374151",
            "footnote_marker_color": self.footnote_marker_color_var.get().strip() or "#111827",
            "table_row_stripes": bool(self.table_stripes_var.get()),
            "table_row_odd_color": self.table_odd_color_var.get().strip() or "#ffffff",
            "table_row_even_color": self.table_even_color_var.get().strip() or "#f3f4f6",
            "table_cell_padding_y_px": table_pad_y,
            "table_cell_padding_x_px": table_pad_x,
            "image_scale": image_scale,
            "show_horizontal_rules": bool(self.show_horizontal_rules_var.get()),
        }
        for level in range(1, 7):
            kwargs[f"heading_h{level}_color"] = self.heading_color_vars[level].get().strip()
            kwargs[f"heading_h{level}_size_px"] = _safe_int(
                self.heading_size_vars[level].get(),
                getattr(self.config.style, f"heading_h{level}_size_px"),
            )

        return StyleOptions(**kwargs)

    def _collect_export(self) -> ExportOptions:
        output_format = self.format_var.get().strip() or "PDF"
        profile = (self.pdf_compression_profile_var.get().strip() or "balanced").lower()
        if profile not in PDF_COMPRESSION_PROFILES:
            profile = "balanced"
        compression = CompressionOptions(
            enabled=bool(self.pdf_compression_enabled_var.get()),
            profile=profile,
            remove_metadata=bool(self.pdf_remove_metadata_var.get()),
        )
        return ExportOptions(output_format=output_format, page_size="A4", compression=compression)

    # --- theme ----------------------------------------------------------------

    def _apply_theme(self) -> None:
        apply_ui_theme(self.root, self.ui_theme_var.get())
        if hasattr(self, "left_canvas"):
            self.left_canvas.configure(bg=self.root.cget("bg"))
        self.status_var.set(f"UI theme applied: {self.ui_theme_var.get()}")

    # --- preview / convert ----------------------------------------------------

    def _preview_html(self) -> None:
        md_path = self._require_markdown_path()
        if md_path is None:
            return
        try:
            style = self._collect_style()
            export = ExportOptions(output_format="HTML", page_size="A4")
            result = self.service.convert(
                markdown_file=md_path,
                output_directory=Path(self.output_dir_var.get()),
                style=style, export=export,
            )
            if result.html_path:
                webbrowser.open(result.html_path.as_uri())
                self.status_var.set(f"Preview: {result.html_path}")
        except Exception as exc:
            logger.exception("HTML preview failed")
            messagebox.showerror("Error", f"Cannot preview: {exc}")
            self.status_var.set("Preview failed")

    @staticmethod
    def _detect_markdown_features(markdown_text: str) -> dict[str, bool]:
        return {
            "blockquote": bool(re.search(r"(?m)^\s{0,3}>\s+", markdown_text)),
            "unordered_list": bool(re.search(r"(?m)^\s*[-*+]\s+", markdown_text)),
            "ordered_list": bool(re.search(r"(?m)^\s*\d+\.\s+", markdown_text)),
            "table": "|" in markdown_text and bool(re.search(r"(?m)^\s*\|?.*\|.*$", markdown_text)),
            "code_fence": bool(re.search(r"(?m)^\s*(```|~~~)", markdown_text)),
            "hr": bool(re.search(r"(?m)^\s{0,3}([-*_])\s*(\1\s*){2,}$", markdown_text)),
            "footnote": "[^" in markdown_text,
            "image": "![" in markdown_text,
            "link": bool(re.search(r"\[[^\]]+\]\([^\)]+\)", markdown_text)),
        }

    def _build_generated_test_markdown(self, source_markdown: str) -> str:
        max_heading = max(1, min(6, self._detect_markdown_max_heading_level_from_text(source_markdown)))
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
            lines.extend([
                "| Column A | Column B |",
                "|---|---|",
                "| Value 1 | Value 2 |",
                "",
            ])
        if features["code_fence"]:
            lines.extend([
                "```python",
                "def greet(name: str) -> str:",
                '    return f"Hello {name}"',
                "",
                'print(greet("world"))',
                "```",
                "",
            ])
        if features["hr"]:
            lines.extend(["---", ""])
        if features["image"]:
            lines.extend(["![Example image](image.png)", ""])
        if features["footnote"]:
            lines.extend(["Text with footnote[^n1].", "", "[^n1]: Auto-generated footnote example.", ""])

        return "\n".join(lines).strip() or _SANDBOX_MARKDOWN

    def _resolve_preview_output_dir(self) -> Path:
        configured = self.output_dir_var.get().strip()
        if configured:
            return Path(configured)
        raw_md = self.markdown_var.get().strip()
        if raw_md:
            return default_output_dir(Path(raw_md))
        return default_output_dir(None)

    def _open_browser_preview(self, preview_path: Path, force_refresh: bool) -> None:
        preview_url = preview_path.as_uri()
        if force_refresh:
            preview_url = f"{preview_url}?v={time.time_ns()}"
        webbrowser.open(preview_url, new=0)

    @staticmethod
    def _detect_markdown_max_heading_level_from_text(markdown_text: str) -> int:
        max_level = 0
        in_fence = False
        fence_token = ""

        for raw_line in markdown_text.splitlines():
            stripped = raw_line.lstrip()
            if stripped.startswith("```") or stripped.startswith("~~~"):
                token = stripped[:3]
                if in_fence and token == fence_token:
                    in_fence = False
                    fence_token = ""
                elif not in_fence:
                    in_fence = True
                    fence_token = token
                continue
            if in_fence:
                continue
            match = re.match(r"^\s{0,3}(#{1,6})\s+\S", raw_line)
            if not match:
                continue
            level = len(match.group(1))
            if level > max_level:
                max_level = level

        return max_level if max_level > 0 else 3

    def _preview_test_html(self) -> None:
        md_path = self._require_markdown_path()
        if md_path is None:
            return
        try:
            style = self._collect_style()
            source_markdown = md_path.read_text(encoding="utf-8")
            markdown_text = self._build_generated_test_markdown(source_markdown)
            html = build_html_from_markdown(
                markdown_text=markdown_text, style=style,
                page_size="A4", title="Style test preview",
                assets_root=md_path.parent,
            )

            output_dir = self._resolve_preview_output_dir()
            output_dir.mkdir(parents=True, exist_ok=True)
            preview_path = output_dir / "_style_preview_test.html"
            preview_path.write_text(html, encoding="utf-8")
            self.live_preview_path = preview_path

            self._open_browser_preview(preview_path, force_refresh=True)
            self.status_var.set(f"Test preview generated: H1-H{self.detected_heading_max_level}")
        except Exception as exc:
            logger.exception("Test preview generation failed")
            messagebox.showerror("Test preview error", str(exc))
            self.status_var.set("Test preview generation failed")

    def _convert(self) -> None:
        md_path = self._require_markdown_path()
        if md_path is None:
            return

        output_dir = Path(self.output_dir_var.get()) if self.output_dir_var.get().strip() else default_output_dir(md_path)
        style = self._collect_style()
        export = self._collect_export()

        try:
            result = self.service.convert(
                markdown_file=md_path, output_directory=output_dir,
                style=style, export=export,
            )
            produced = [str(path) for path in [result.pdf_path, result.html_path] if path]
            page_info = ""
            if result.pdf_page_count is not None:
                page_info = f"\nPDF pages: {result.pdf_page_count}"

            compression_info = ""
            if result.pdf_size_before_bytes is not None and result.pdf_size_after_bytes is not None:
                before_kb = result.pdf_size_before_bytes / 1024
                after_kb = result.pdf_size_after_bytes / 1024
                if result.pdf_size_before_bytes > 0:
                    gain_pct = ((result.pdf_size_before_bytes - result.pdf_size_after_bytes) / result.pdf_size_before_bytes) * 100
                else:
                    gain_pct = 0.0
                compression_info = f"\nPDF size: {before_kb:.1f}KB -> {after_kb:.1f}KB ({gain_pct:.1f}%)"
                if result.pdf_size_after_bytes >= result.pdf_size_before_bytes and not result.pdf_compression_tool:
                    compression_info += "\nTip: install qpdf for better compression on some documents."

            status_msg = "Conversion complete"
            if result.pdf_page_count is not None:
                status_msg += f" ({result.pdf_page_count} pages)"
            if result.pdf_size_before_bytes and result.pdf_size_after_bytes:
                before_kb = result.pdf_size_before_bytes / 1024
                after_kb = result.pdf_size_after_bytes / 1024
                status_msg += f" | PDF {before_kb:.1f}KB -> {after_kb:.1f}KB"
            self.status_var.set(status_msg)

            messagebox.showinfo("Success", "Generated files:\n" + "\n".join(produced) + page_info + compression_info)
        except Exception as exc:
            logger.exception("Conversion failed")
            msg = str(exc)
            if "cannot load library" in msg.lower() or "pango" in msg.lower():
                msg += "\n\nOn Windows, check the Pango/MSYS2 installation for WeasyPrint (see README)."
            messagebox.showerror("Conversion error", msg)
            self.status_var.set("Conversion failed")

    def _require_markdown_path(self) -> Path | None:
        raw = self.markdown_var.get().strip()
        if not raw:
            messagebox.showwarning("File required", "Select a Markdown file before continuing.")
            return None
        path = Path(raw)
        if not path.exists() or not path.is_file():
            messagebox.showwarning("File not found", "The selected Markdown file was not found.")
            return None
        if path.suffix.lower() != ".md":
            messagebox.showwarning("Invalid format", "The file must have a .md extension.")
            return None
        return path
