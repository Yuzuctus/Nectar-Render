"""Main GUI application for Nectar Render."""

from __future__ import annotations

import logging
import re
import tkinter as tk
from pathlib import Path
from tkinter import filedialog, font as tkfont, messagebox, ttk

from ..config import (
    AppConfig,
    CompressionOptions,
    ExportOptions,
    PDF_COMPRESSION_PROFILES,
    StyleOptions,
    DEFAULT_BODY_FONT,
    DEFAULT_CODE_FONT,
    DEFAULT_HEADING_COLOR,
    FALLBACK_FONTS,
)
from ..converter.markdown_parser import invalidate_image_index_cache
from ..services.conversion_service import ConversionService
from ..utils.paths import application_data_dir, default_output_dir
from ..utils.converters import safe_int, safe_float
from .bindings import OptionChangeController, UiBindingManager
from .controllers import ConversionController, PreviewController
from .panels import (
    AdvancedTypographyPanel,
    BasicSettingsPanel,
    CodePanel,
    FootnotePanel,
    MarginsPanel,
    TablePanel,
    TypographyPanel,
)
from .state_manager import StateManager
from .theme import apply_ui_theme

logger = logging.getLogger(__name__)

PREVIEW_DEBOUNCE_MS = 300

_PRESETS_FILENAME = "presets.json"
_STATE_FILENAME = "last_state.json"


class NectarRenderApp:
    """Tkinter GUI for Nectar Render."""

    def __init__(self, root: tk.Tk) -> None:
        self.root = root
        self.root.title("Nectar Render")
        self.root.geometry("1120x820")

        self.config = AppConfig()
        self.service = ConversionService()
        self._destroyed = False

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
        self.binding_manager = UiBindingManager(self.root)
        self.option_change_controller = OptionChangeController(
            self.root, self.state_mgr, PREVIEW_DEBOUNCE_MS
        )

        # Register Tk variables with state manager
        self.state_mgr.set_tk_vars(
            tk_vars=self._tk_var_dict(),
            status_var=self.status_var,
            ensure_font_option=self._ensure_font_option,
        )
        self.preview_controller = PreviewController(
            self.service,
            require_markdown_path=self._require_markdown_path,
            collect_style=self._collect_style,
            configured_output_dir_getter=self.output_dir_var.get,
            page_size_getter=self.page_size_var.get,
            heading_level_detector=self._detect_markdown_max_heading_level_from_text,
            set_status=self.status_var.set,
        )
        self.conversion_controller = ConversionController(
            self.root,
            self.service,
            require_markdown_path=self._require_markdown_path,
            collect_style=self._collect_style,
            collect_export=self._collect_export,
            configured_output_dir_getter=self.output_dir_var.get,
            set_status=self.status_var.set,
            set_convert_enabled=self._set_convert_enabled,
        )

        apply_ui_theme(self.root, self.ui_theme_var.get())
        self._build_ui()
        self._configure_bindings()
        self._refresh_preset_list()
        self.state_mgr.load_last_state()
        self._refresh_heading_controls_from_markdown()
        self.state_mgr.initialize_history()

    # --- Tk variables ---------------------------------------------------------

    def _create_tk_vars(self) -> None:
        cfg = self.config

        self.markdown_var = tk.StringVar(value="")
        self.output_dir_var = tk.StringVar(value=str(default_output_dir(None)))
        self.format_var = tk.StringVar(value="PDF")
        self.page_size_var = tk.StringVar(value=cfg.export.page_size)
        self.pdf_compression_enabled_var = tk.BooleanVar(
            value=cfg.export.compression.enabled
        )
        self.pdf_compression_profile_var = tk.StringVar(
            value=cfg.export.compression.profile
        )
        self.pdf_remove_metadata_var = tk.BooleanVar(
            value=cfg.export.compression.remove_metadata
        )
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
        self.footer_color_var = tk.StringVar(value=cfg.style.footer_color)
        footer_align_value = (cfg.style.footer_align or "right").lower()
        self.footer_align_var = tk.StringVar(
            value="Center" if footer_align_value == "center" else "Right"
        )
        self.include_notes_var = tk.BooleanVar(value=cfg.style.include_footnotes)
        self.footnote_size_var = tk.DoubleVar(value=cfg.style.footnote_font_size)
        self.footnote_text_color_var = tk.StringVar(value=cfg.style.footnote_text_color)
        self.footnote_marker_color_var = tk.StringVar(
            value=cfg.style.footnote_marker_color
        )
        self.table_stripes_var = tk.BooleanVar(value=cfg.style.table_row_stripes)
        self.table_odd_color_var = tk.StringVar(value=cfg.style.table_row_odd_color)
        self.table_even_color_var = tk.StringVar(value=cfg.style.table_row_even_color)
        self.table_pad_y_var = tk.IntVar(value=cfg.style.table_cell_padding_y_px)
        self.table_pad_x_var = tk.IntVar(value=cfg.style.table_cell_padding_x_px)
        self.image_scale_var = tk.DoubleVar(
            value=round(cfg.style.image_scale * 100.0, 1)
        )
        self.show_horizontal_rules_var = tk.BooleanVar(
            value=cfg.style.show_horizontal_rules
        )
        self.preset_var = tk.StringVar(value="")
        self.ui_theme_var = tk.StringVar(value="Light")
        self.status_var = tk.StringVar(value="Ready")

    def _tk_var_dict(self) -> dict[str, tk.Variable]:
        """Return a name->Variable mapping for all state-tracked variables."""
        d: dict[str, tk.Variable] = {
            "markdown_var": self.markdown_var,
            "output_dir_var": self.output_dir_var,
            "format_var": self.format_var,
            "page_size_var": self.page_size_var,
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
            "footer_color_var": self.footer_color_var,
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

        ttk.Label(header, text="Nectar Render", style="Title.TLabel").grid(
            row=0, column=0, sticky="w"
        )
        ttk.Label(
            header,
            text="Markdown to PDF / HTML desktop converter",
            style="Muted.TLabel",
        ).grid(row=1, column=0, sticky="w", pady=(2, 0))

        header_actions = ttk.Frame(header)
        header_actions.grid(row=0, column=1, rowspan=2, sticky="e")
        ttk.Button(
            header_actions,
            text="Preview HTML",
            command=self.preview_controller.preview_html,
        ).pack(side=tk.LEFT)
        ttk.Button(
            header_actions,
            text="Test preview",
            command=self.preview_controller.preview_test_html,
        ).pack(side=tk.LEFT, padx=(8, 0))
        ttk.Button(
            header_actions,
            text="Convert",
            command=self.conversion_controller.start_conversion,
            style="Primary.TButton",
        ).pack(side=tk.LEFT, padx=(8, 0))

        # Scrollable panel
        left_shell = ttk.Frame(container, padding=2)
        left_shell.grid(row=1, column=0, sticky="nsew")

        self.left_canvas = tk.Canvas(
            left_shell, highlightthickness=0, borderwidth=0, bg=self.root.cget("bg")
        )
        left_scrollbar = ttk.Scrollbar(
            left_shell, orient=tk.VERTICAL, command=self.left_canvas.yview
        )
        self.left_scrollbar = left_scrollbar
        self.left_scroll_enabled = False
        self.left_canvas.configure(yscrollcommand=left_scrollbar.set)
        left_scrollbar.pack(side=tk.RIGHT, fill=tk.Y)
        self.left_canvas.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)

        left_panel = ttk.Frame(self.left_canvas)
        left_panel.columnconfigure(0, weight=1)
        self.left_canvas_window = self.left_canvas.create_window(
            (0, 0), window=left_panel, anchor="nw"
        )

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

        left_panel.bind("<Configure>", _sync_left_scrollregion)
        self.left_canvas.bind("<Configure>", _sync_left_width)

        # --- Panels (delegated to ui/panels.py) --------------------------------
        def _on_font_commit(_value: str) -> None:
            self._update_font_previews()

        self._basic_panel = BasicSettingsPanel(
            left_panel,
            self.root,
            markdown_var=self.markdown_var,
            output_dir_var=self.output_dir_var,
            format_var=self.format_var,
            page_size_var=self.page_size_var,
            ui_theme_var=self.ui_theme_var,
            preset_var=self.preset_var,
            pdf_compression_enabled_var=self.pdf_compression_enabled_var,
            pdf_compression_profile_var=self.pdf_compression_profile_var,
            pdf_remove_metadata_var=self.pdf_remove_metadata_var,
            on_browse_markdown=self._select_markdown_file,
            on_browse_output=self._select_output_directory,
            on_apply_theme=self._apply_theme,
            on_save_preset=self._save_preset,
            on_load_preset=self._load_selected_preset,
        )
        self.preset_combo = self._basic_panel.preset_combo

        self._typography_panel = TypographyPanel(
            left_panel,
            self.root,
            body_font_var=self.body_font_var,
            heading_font_var=self.heading_font_var,
            body_size_var=self.body_size_var,
            line_height_var=self.line_height_var,
            available_fonts=self.available_fonts,
            on_font_commit=_on_font_commit,
        )
        self.body_font_combo = self._typography_panel.body_font_combo
        self.body_font_preview = self._typography_panel.body_font_preview
        self.heading_font_combo = self._typography_panel.heading_font_combo
        self.heading_font_preview = self._typography_panel.heading_font_preview

        MarginsPanel(
            left_panel,
            margin_top_var=self.margin_top_var,
            margin_right_var=self.margin_right_var,
            margin_bottom_var=self.margin_bottom_var,
            margin_left_var=self.margin_left_var,
        )

        # Advanced panel (two columns)
        self.advanced_panel = ttk.Frame(left_panel)
        self.advanced_panel.pack(fill=tk.BOTH, expand=True, pady=(0, 8))
        self.advanced_panel.columnconfigure(0, weight=1)
        self.advanced_panel.columnconfigure(1, weight=1)

        self._adv_typo_panel = AdvancedTypographyPanel(
            self.advanced_panel,
            heading_color_var=self.heading_color_var,
            heading_font_var=self.heading_font_var,
            heading_color_vars=self.heading_color_vars,
            heading_size_vars=self.heading_size_vars,
        )
        self.heading_level_rows = self._adv_typo_panel.heading_level_rows
        self.heading_detected_label = self._adv_typo_panel.heading_detected_label

        self._code_panel = CodePanel(
            self.advanced_panel,
            self.root,
            code_theme_var=self.code_theme_var,
            code_font_var=self.code_font_var,
            code_size_var=self.code_size_var,
            code_line_height_var=self.code_line_height_var,
            available_fonts=self.available_fonts,
            on_font_commit=_on_font_commit,
        )
        self.code_font_combo = self._code_panel.code_font_combo
        self.code_font_preview = self._code_panel.code_font_preview

        TablePanel(
            self.advanced_panel,
            table_stripes_var=self.table_stripes_var,
            table_odd_color_var=self.table_odd_color_var,
            table_even_color_var=self.table_even_color_var,
            table_pad_y_var=self.table_pad_y_var,
            table_pad_x_var=self.table_pad_x_var,
            image_scale_var=self.image_scale_var,
            show_horizontal_rules_var=self.show_horizontal_rules_var,
        )

        FootnotePanel(
            self.advanced_panel,
            include_notes_var=self.include_notes_var,
            footnote_size_var=self.footnote_size_var,
            footnote_text_color_var=self.footnote_text_color_var,
            footnote_marker_color_var=self.footnote_marker_color_var,
            footer_color_var=self.footer_color_var,
            footer_align_var=self.footer_align_var,
            footer_text_var=self.footer_text_var,
        )

        # Status bar
        ttk.Label(container, textvariable=self.status_var).grid(
            row=2, column=0, sticky="w", pady=(8, 0)
        )

        help_text = (
            "Page breaks: <!-- pagebreak -->, \\pagebreak, [[PAGEBREAK]]\n"
            "All settings are visible. Use scrollbars for long paths."
        )
        ttk.Label(
            container, text=help_text, justify=tk.LEFT, style="Muted.TLabel"
        ).grid(row=3, column=0, sticky="w", pady=(6, 0))

    # --- bindings -------------------------------------------------------------

    def _configure_bindings(self) -> None:
        self.binding_manager.trace_variables(
            [self.markdown_var], self._on_markdown_path_changed
        )
        self.binding_manager.trace_variables(
            [self.body_font_var, self.heading_font_var, self.code_font_var],
            lambda *_: self._update_font_previews(),
        )
        self.binding_manager.trace_variables(
            self._tk_var_dict().values(),
            self.option_change_controller.on_option_changed,
        )
        self.binding_manager.bind_root("<Control-z>", self.state_mgr.undo_action)
        self.binding_manager.bind_root("<Control-Alt-z>", self.state_mgr.redo_action)
        self.binding_manager.bind_root("<MouseWheel>", self._on_left_mousewheel)
        self.binding_manager.bind_root("<Button-4>", self._on_left_mousewheel)
        self.binding_manager.bind_root("<Button-5>", self._on_left_mousewheel)
        self.binding_manager.bind_root("<Destroy>", self._on_root_destroy)
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

    def _on_left_mousewheel(self, event: tk.Event[tk.Misc]) -> str:
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

    def _on_root_destroy(self, event: tk.Event[tk.Misc]) -> None:
        if event.widget is not self.root or self._destroyed:
            return
        self._destroyed = True
        self.option_change_controller.dispose()
        self.binding_manager.dispose()
        for widget in (
            getattr(self, "body_font_combo", None),
            getattr(self, "heading_font_combo", None),
            getattr(self, "code_font_combo", None),
        ):
            if widget is not None:
                widget.destroy()

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
            if (
                candidate.exists()
                and candidate.is_file()
                and candidate.suffix.lower() == ".md"
            ):
                max_level = self._detect_markdown_max_heading_level(candidate)

        self.detected_heading_max_level = max(1, min(6, max_level))
        for level, widgets in self.heading_level_rows.items():
            if level <= self.detected_heading_max_level:
                for widget in widgets:
                    widget.grid()
            else:
                for widget in widgets:
                    widget.grid_remove()

        self.heading_detected_label.configure(
            text=f"Detected headings: H1 to H{self.detected_heading_max_level}"
        )

    def _on_markdown_path_changed(self, *_args: object) -> None:
        invalidate_image_index_cache()
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
        path = filedialog.askopenfilename(
            filetypes=[("Markdown", "*.md"), ("All files", "*.*")]
        )
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
        body_font_size = safe_int(self.body_size_var.get(), 12)
        code_font_size = safe_int(self.code_size_var.get(), 11)
        code_line_height = safe_float(
            self.code_line_height_var.get(), self.config.style.code_line_height
        )
        line_height = safe_float(self.line_height_var.get(), 1.5)
        margin_top = safe_float(self.margin_top_var.get(), 25.4)
        margin_right = safe_float(self.margin_right_var.get(), 25.4)
        margin_bottom = safe_float(self.margin_bottom_var.get(), 25.4)
        margin_left = safe_float(self.margin_left_var.get(), 25.4)
        footer_align = (
            "center"
            if self.footer_align_var.get().strip().lower() == "center"
            else "right"
        )
        footnote_size = safe_float(self.footnote_size_var.get(), 9.0)
        table_pad_y = safe_int(
            self.table_pad_y_var.get(), self.config.style.table_cell_padding_y_px
        )
        table_pad_x = safe_int(
            self.table_pad_x_var.get(), self.config.style.table_cell_padding_x_px
        )
        image_scale_pct = safe_float(
            self.image_scale_var.get(), self.config.style.image_scale * 100.0
        )
        image_scale = max(0.4, min(1.0, image_scale_pct / 100.0))

        kwargs: dict[str, object] = {
            "body_font": self.body_font_var.get().strip() or DEFAULT_BODY_FONT,
            "body_font_size": body_font_size,
            "line_height": line_height,
            "heading_font": self.heading_font_var.get().strip() or DEFAULT_BODY_FONT,
            "heading_color": self.heading_color_var.get().strip()
            or DEFAULT_HEADING_COLOR,
            "code_font": self.code_font_var.get().strip() or DEFAULT_CODE_FONT,
            "code_font_size": code_font_size,
            "code_line_height": code_line_height,
            "code_theme": self.code_theme_var.get().strip() or "default",
            "margin_top_mm": margin_top,
            "margin_right_mm": margin_right,
            "margin_bottom_mm": margin_bottom,
            "margin_left_mm": margin_left,
            "footer_text": self.footer_text_var.get().strip(),
            "footer_color": self.footer_color_var.get().strip() or "#6b7280",
            "footer_align": footer_align,
            "include_footnotes": bool(self.include_notes_var.get()),
            "footnote_font_size": footnote_size,
            "footnote_text_color": self.footnote_text_color_var.get().strip()
            or "#374151",
            "footnote_marker_color": self.footnote_marker_color_var.get().strip()
            or "#111827",
            "table_row_stripes": bool(self.table_stripes_var.get()),
            "table_row_odd_color": self.table_odd_color_var.get().strip() or "#ffffff",
            "table_row_even_color": self.table_even_color_var.get().strip()
            or "#f3f4f6",
            "table_cell_padding_y_px": table_pad_y,
            "table_cell_padding_x_px": table_pad_x,
            "image_scale": image_scale,
            "show_horizontal_rules": bool(self.show_horizontal_rules_var.get()),
        }
        for level in range(1, 7):
            kwargs[f"heading_h{level}_color"] = (
                self.heading_color_vars[level].get().strip()
            )
            kwargs[f"heading_h{level}_size_px"] = safe_int(
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
        page_size = self.page_size_var.get().strip() or "A4"
        return ExportOptions(
            output_format=output_format, page_size=page_size, compression=compression
        )

    # --- theme ----------------------------------------------------------------

    def _apply_theme(self) -> None:
        apply_ui_theme(self.root, self.ui_theme_var.get())
        if hasattr(self, "left_canvas"):
            self.left_canvas.configure(bg=self.root.cget("bg"))
        self.status_var.set(f"UI theme applied: {self.ui_theme_var.get()}")

    @staticmethod
    def _detect_markdown_max_heading_level_from_text(markdown_text: str) -> int:
        from ..utils.markdown import iter_lines_outside_fences

        max_level = 0
        for line, in_fence in iter_lines_outside_fences(markdown_text):
            if in_fence:
                continue
            match = re.match(r"^\s{0,3}(#{1,6})\s+\S", line)
            if not match:
                continue
            level = len(match.group(1))
            if level > max_level:
                max_level = level

        return max_level if max_level > 0 else 3

    def _set_convert_enabled(self, enabled: bool) -> None:
        state = "normal" if enabled else "disabled"
        for widget in self.root.winfo_children():
            self._set_button_state_recursive(widget, state)

    @staticmethod
    def _set_button_state_recursive(widget: tk.Widget, state: str) -> None:
        if isinstance(widget, ttk.Button):
            try:
                widget.configure(state=state)
            except tk.TclError:
                pass
        for child in widget.winfo_children():
            NectarRenderApp._set_button_state_recursive(child, state)

    def _require_markdown_path(self) -> Path | None:
        raw = self.markdown_var.get().strip()
        if not raw:
            messagebox.showwarning(
                "File required", "Select a Markdown file before continuing."
            )
            return None
        path = Path(raw)
        if not path.exists() or not path.is_file():
            messagebox.showwarning(
                "File not found", "The selected Markdown file was not found."
            )
            return None
        if path.suffix.lower() != ".md":
            messagebox.showwarning(
                "Invalid format", "The file must have a .md extension."
            )
            return None
        return path
