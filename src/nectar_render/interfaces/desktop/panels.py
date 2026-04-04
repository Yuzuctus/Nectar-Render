"""Reusable UI panel builders for the Nectar Render application."""

from __future__ import annotations

import tkinter as tk
from collections.abc import Callable
from tkinter import ttk

from ...config import (
    OUTPUT_FORMATS,
    PAGE_SIZES,
    PDF_COMPRESSION_PROFILES,
    UI_THEMES,
)
from ...adapters.rendering.highlight import list_available_styles
from .widgets import FontAutocomplete


class BasicSettingsPanel:
    """File selection, output format, presets, and compression options."""

    def __init__(
        self,
        parent: ttk.Frame,
        *,
        markdown_var: tk.StringVar,
        output_dir_var: tk.StringVar,
        format_var: tk.StringVar,
        page_size_var: tk.StringVar,
        ui_theme_var: tk.StringVar,
        preset_var: tk.StringVar,
        pdf_compression_enabled_var: tk.BooleanVar,
        pdf_compression_profile_var: tk.StringVar,
        pdf_remove_metadata_var: tk.BooleanVar,
        on_browse_markdown: Callable[[], None],
        on_browse_output: Callable[[], None],
        on_apply_theme: Callable[[], None],
        on_save_preset: Callable[[], None],
        on_load_preset: Callable[[], None],
    ) -> None:
        box = ttk.LabelFrame(parent, text="Basic settings", padding=8)
        box.pack(fill=tk.X, pady=(0, 6))
        box.columnconfigure(1, weight=1)
        self.frame = box

        ttk.Label(box, text="Markdown file (.md)").grid(row=0, column=0, sticky="w")
        self.markdown_entry = ttk.Entry(box, textvariable=markdown_var)
        self.markdown_entry.grid(
            row=0, column=1, columnspan=4, sticky="ew", padx=(8, 8)
        )
        ttk.Button(box, text="Browse", command=on_browse_markdown).grid(row=0, column=5)

        ttk.Label(box, text="Output folder").grid(
            row=1, column=0, sticky="w", pady=(6, 0)
        )
        self.output_dir_entry = ttk.Entry(box, textvariable=output_dir_var)
        self.output_dir_entry.grid(
            row=1, column=1, columnspan=4, sticky="ew", padx=(8, 8), pady=(6, 0)
        )
        ttk.Button(box, text="Choose", command=on_browse_output).grid(
            row=1, column=5, pady=(6, 0)
        )

        options_row = ttk.Frame(box)
        options_row.grid(row=2, column=0, columnspan=6, sticky="ew", pady=(8, 0))

        ttk.Label(options_row, text="Format").pack(side=tk.LEFT)
        ttk.Combobox(
            options_row,
            textvariable=format_var,
            values=OUTPUT_FORMATS,
            state="readonly",
            width=10,
        ).pack(side=tk.LEFT, padx=(8, 16))

        ttk.Label(options_row, text="Page size").pack(side=tk.LEFT)
        ttk.Combobox(
            options_row,
            textvariable=page_size_var,
            values=PAGE_SIZES,
            state="readonly",
            width=8,
        ).pack(side=tk.LEFT, padx=(8, 16))

        ttk.Label(options_row, text="UI Theme").pack(side=tk.LEFT)
        ttk.Combobox(
            options_row,
            textvariable=ui_theme_var,
            values=UI_THEMES,
            state="readonly",
            width=10,
        ).pack(side=tk.LEFT, padx=(8, 8))
        ttk.Button(options_row, text="Apply", command=on_apply_theme).pack(
            side=tk.LEFT, padx=(0, 16)
        )

        ttk.Label(options_row, text="Preset").pack(side=tk.LEFT)
        self.preset_combo = ttk.Combobox(
            options_row, textvariable=preset_var, values=[], width=24
        )
        self.preset_combo.pack(side=tk.LEFT, padx=(8, 8))
        ttk.Button(options_row, text="Save", command=on_save_preset).pack(side=tk.LEFT)
        ttk.Button(options_row, text="Load", command=on_load_preset).pack(
            side=tk.LEFT, padx=(8, 0)
        )

        compression_row = ttk.Frame(box)
        compression_row.grid(row=3, column=0, columnspan=6, sticky="ew", pady=(8, 0))
        ttk.Checkbutton(
            compression_row,
            text="Automatic PDF compression",
            variable=pdf_compression_enabled_var,
        ).pack(side=tk.LEFT)
        ttk.Label(compression_row, text="Profile").pack(side=tk.LEFT, padx=(12, 0))
        ttk.Combobox(
            compression_row,
            textvariable=pdf_compression_profile_var,
            values=PDF_COMPRESSION_PROFILES,
            state="readonly",
            width=10,
        ).pack(side=tk.LEFT, padx=(8, 8))
        ttk.Checkbutton(
            compression_row, text="Remove metadata", variable=pdf_remove_metadata_var
        ).pack(side=tk.LEFT, padx=(8, 0))


class TypographyPanel:
    """Body and heading font selection with previews."""

    def __init__(
        self,
        parent: ttk.Frame,
        root: tk.Tk,
        *,
        body_font_var: tk.StringVar,
        heading_font_var: tk.StringVar,
        body_size_var: tk.IntVar,
        line_height_var: tk.DoubleVar,
        available_fonts: list[str],
        on_font_commit: Callable[..., object],
    ) -> None:
        box = ttk.LabelFrame(parent, text="Typography", padding=10)
        box.pack(fill=tk.X, pady=(0, 8))
        self.frame = box

        ttk.Label(box, text="Body font").grid(row=0, column=0, sticky="w")
        body_font_cell = ttk.Frame(box)
        body_font_cell.grid(row=0, column=1, sticky="w", padx=(8, 8))
        self.body_font_combo = FontAutocomplete(
            root=root,
            parent=body_font_cell,
            textvariable=body_font_var,
            values=available_fonts,
            width=22,
            on_commit=on_font_commit,
        )
        self.body_font_combo.frame.pack(side=tk.LEFT)
        self.body_font_preview = ttk.Label(body_font_cell, text="AaBb 123", width=10)
        self.body_font_preview.pack(side=tk.LEFT, padx=(8, 0))

        ttk.Label(box, text="Size").grid(row=0, column=2, sticky="w")
        ttk.Spinbox(box, from_=8, to=24, textvariable=body_size_var, width=7).grid(
            row=0, column=3, sticky="w", padx=(8, 8)
        )
        ttk.Label(box, text="Line height").grid(row=0, column=4, sticky="w")
        ttk.Spinbox(
            box, from_=1.0, to=2.4, increment=0.1, textvariable=line_height_var, width=7
        ).grid(row=0, column=5, sticky="w", padx=(8, 0))

        ttk.Label(box, text="Heading font").grid(
            row=1, column=0, sticky="w", pady=(8, 0)
        )
        heading_font_cell = ttk.Frame(box)
        heading_font_cell.grid(row=1, column=1, sticky="w", padx=(8, 8), pady=(8, 0))
        self.heading_font_combo = FontAutocomplete(
            root=root,
            parent=heading_font_cell,
            textvariable=heading_font_var,
            values=available_fonts,
            width=22,
            on_commit=on_font_commit,
        )
        self.heading_font_combo.frame.pack(side=tk.LEFT)
        self.heading_font_preview = ttk.Label(
            heading_font_cell, text="AaBb 123", width=10
        )
        self.heading_font_preview.pack(side=tk.LEFT, padx=(8, 0))


class MarginsPanel:
    """Page margin controls."""

    def __init__(
        self,
        parent: ttk.Frame,
        *,
        margin_top_var: tk.DoubleVar,
        margin_right_var: tk.DoubleVar,
        margin_bottom_var: tk.DoubleVar,
        margin_left_var: tk.DoubleVar,
    ) -> None:
        box = ttk.LabelFrame(parent, text="Page margins", padding=10)
        box.pack(fill=tk.X, pady=(0, 8))
        self.frame = box

        ttk.Label(box, text="Top").grid(row=0, column=0, sticky="w")
        ttk.Spinbox(
            box, from_=5, to=50, increment=0.5, textvariable=margin_top_var, width=7
        ).grid(row=0, column=1, sticky="w", padx=(8, 12))
        ttk.Label(box, text="Right").grid(row=0, column=2, sticky="w")
        ttk.Spinbox(
            box, from_=5, to=50, increment=0.5, textvariable=margin_right_var, width=7
        ).grid(row=0, column=3, sticky="w", padx=(8, 12))
        ttk.Label(box, text="Bottom").grid(row=1, column=0, sticky="w", pady=(8, 0))
        ttk.Spinbox(
            box, from_=5, to=50, increment=0.5, textvariable=margin_bottom_var, width=7
        ).grid(row=1, column=1, sticky="w", padx=(8, 12), pady=(8, 0))
        ttk.Label(box, text="Left").grid(row=1, column=2, sticky="w", pady=(8, 0))
        ttk.Spinbox(
            box, from_=5, to=50, increment=0.5, textvariable=margin_left_var, width=7
        ).grid(row=1, column=3, sticky="w", padx=(8, 12), pady=(8, 0))


class AdvancedTypographyPanel:
    """Per-heading color/size controls and global heading color."""

    def __init__(
        self,
        parent: ttk.Frame,
        *,
        heading_color_var: tk.StringVar,
        heading_font_var: tk.StringVar,
        heading_color_vars: dict[int, tk.StringVar],
        heading_size_vars: dict[int, tk.IntVar],
    ) -> None:
        box = ttk.LabelFrame(parent, text="Advanced typography", padding=10)
        box.grid(row=0, column=0, sticky="nsew", pady=(0, 8), padx=(0, 6))
        box.columnconfigure(1, weight=1)
        self.frame = box

        ttk.Label(box, text="Global heading color").grid(row=0, column=0, sticky="w")
        ttk.Entry(box, textvariable=heading_color_var, width=10).grid(
            row=0, column=1, sticky="w", padx=(8, 12)
        )
        self.heading_detected_label = ttk.Label(box, text="", style="Muted.TLabel")
        self.heading_detected_label.grid(
            row=0, column=2, columnspan=2, sticky="w", padx=(8, 0)
        )

        ttk.Label(box, text="Level").grid(row=1, column=0, sticky="w", pady=(8, 0))
        ttk.Label(box, text="Font").grid(row=1, column=1, sticky="w", pady=(8, 0))
        ttk.Label(box, text="Size").grid(row=1, column=2, sticky="w", pady=(8, 0))
        ttk.Label(box, text="Color").grid(row=1, column=3, sticky="w", pady=(8, 0))

        self.heading_level_rows: dict[int, tuple[tk.Widget, ...]] = {}
        for level in range(1, 7):
            row = level + 1
            level_label = ttk.Label(box, text=f"H{level}")
            level_label.grid(row=row, column=0, sticky="w", pady=(8, 0))
            level_font_label = ttk.Label(box, textvariable=heading_font_var, width=24)
            level_font_label.grid(row=row, column=1, sticky="w", pady=(8, 0))
            level_size_spin = ttk.Spinbox(
                box, from_=10, to=96, textvariable=heading_size_vars[level], width=7
            )
            level_size_spin.grid(
                row=row, column=2, sticky="w", padx=(8, 12), pady=(8, 0)
            )
            level_color_entry = ttk.Entry(
                box, textvariable=heading_color_vars[level], width=10
            )
            level_color_entry.grid(
                row=row, column=3, sticky="w", padx=(0, 8), pady=(8, 0)
            )
            self.heading_level_rows[level] = (
                level_label,
                level_font_label,
                level_size_spin,
                level_color_entry,
            )


class CodePanel:
    """Code theme, font, size, and line height."""

    def __init__(
        self,
        parent: ttk.Frame,
        root: tk.Tk,
        *,
        code_theme_var: tk.StringVar,
        code_font_var: tk.StringVar,
        code_size_var: tk.IntVar,
        code_line_height_var: tk.DoubleVar,
        available_fonts: list[str],
        on_font_commit: Callable[..., object],
    ) -> None:
        box = ttk.LabelFrame(parent, text="Code", padding=10)
        box.grid(row=0, column=1, sticky="nsew", pady=(0, 8), padx=(6, 0))
        self.frame = box
        available_code_themes = list_available_styles()

        ttk.Label(box, text="Code theme").grid(row=0, column=0, sticky="w")
        code_theme_cell = ttk.Frame(box)
        code_theme_cell.grid(row=0, column=1, sticky="w", padx=(8, 12))
        self.code_theme_combo = FontAutocomplete(
            root=root,
            parent=code_theme_cell,
            textvariable=code_theme_var,
            values=available_code_themes,
            width=18,
            allow_custom_values=False,
        )
        self.code_theme_combo.frame.pack(side=tk.LEFT)

        ttk.Label(box, text="Code font").grid(row=1, column=0, sticky="w", pady=(8, 0))
        code_font_cell = ttk.Frame(box)
        code_font_cell.grid(row=1, column=1, sticky="ew", padx=(8, 12), pady=(8, 0))
        self.code_font_combo = FontAutocomplete(
            root=root,
            parent=code_font_cell,
            textvariable=code_font_var,
            values=available_fonts,
            width=22,
            on_commit=on_font_commit,
        )
        self.code_font_combo.frame.pack(side=tk.LEFT)
        self.code_font_preview = ttk.Label(code_font_cell, text="AaBb 123", width=10)
        self.code_font_preview.pack(side=tk.LEFT, padx=(8, 0))
        ttk.Label(box, text="Size").grid(row=1, column=2, sticky="w", pady=(8, 0))
        ttk.Spinbox(box, from_=8, to=24, textvariable=code_size_var, width=7).grid(
            row=1, column=3, sticky="w", padx=(8, 0), pady=(8, 0)
        )
        ttk.Label(box, text="Line height").grid(
            row=1, column=4, sticky="w", padx=(14, 0), pady=(8, 0)
        )
        ttk.Spinbox(
            box,
            from_=1.0,
            to=2.4,
            increment=0.1,
            textvariable=code_line_height_var,
            width=7,
        ).grid(row=1, column=5, sticky="w", padx=(8, 0), pady=(8, 0))


class TablePanel:
    """Table striping, padding, image scale, and horizontal rules."""

    def __init__(
        self,
        parent: ttk.Frame,
        *,
        table_stripes_var: tk.BooleanVar,
        table_odd_color_var: tk.StringVar,
        table_even_color_var: tk.StringVar,
        table_pad_y_var: tk.IntVar,
        table_pad_x_var: tk.IntVar,
        image_scale_var: tk.DoubleVar,
        show_horizontal_rules_var: tk.BooleanVar,
    ) -> None:
        box = ttk.LabelFrame(parent, text="Tables & separators", padding=10)
        box.grid(row=1, column=0, sticky="nsew", pady=(0, 8), padx=(0, 6))
        self.frame = box

        ttk.Checkbutton(
            box, text="Alternating row colors (striping)", variable=table_stripes_var
        ).grid(row=0, column=0, columnspan=4, sticky="w")
        ttk.Label(box, text="Odd row color").grid(
            row=1, column=0, sticky="w", pady=(8, 0)
        )
        ttk.Entry(box, textvariable=table_odd_color_var, width=10).grid(
            row=1, column=1, sticky="w", padx=(8, 12), pady=(8, 0)
        )
        ttk.Label(box, text="Even row color").grid(
            row=1, column=2, sticky="w", pady=(8, 0)
        )
        ttk.Entry(box, textvariable=table_even_color_var, width=10).grid(
            row=1, column=3, sticky="w", padx=(8, 0), pady=(8, 0)
        )
        ttk.Label(box, text="Padding Y (px)").grid(
            row=2, column=0, sticky="w", pady=(8, 0)
        )
        ttk.Spinbox(box, from_=0, to=30, textvariable=table_pad_y_var, width=7).grid(
            row=2, column=1, sticky="w", padx=(8, 12), pady=(8, 0)
        )
        ttk.Label(box, text="Padding X (px)").grid(
            row=2, column=2, sticky="w", pady=(8, 0)
        )
        ttk.Spinbox(box, from_=0, to=40, textvariable=table_pad_x_var, width=7).grid(
            row=2, column=3, sticky="w", padx=(8, 0), pady=(8, 0)
        )
        ttk.Label(box, text="Image size (%)").grid(
            row=3, column=0, sticky="w", pady=(8, 0)
        )
        ttk.Scale(
            box, from_=40, to=100, variable=image_scale_var, orient=tk.HORIZONTAL
        ).grid(row=3, column=1, columnspan=2, sticky="ew", padx=(8, 12), pady=(8, 0))
        image_scale_input = ttk.Frame(box)
        image_scale_input.grid(row=3, column=3, sticky="w", pady=(8, 0))
        ttk.Spinbox(
            image_scale_input,
            from_=40,
            to=100,
            increment=0.1,
            textvariable=image_scale_var,
            width=7,
        ).pack(side=tk.LEFT)
        ttk.Label(image_scale_input, text="%", width=2).pack(side=tk.LEFT, padx=(4, 0))
        ttk.Checkbutton(
            box, text="Show horizontal rules (---)", variable=show_horizontal_rules_var
        ).grid(row=4, column=0, columnspan=4, sticky="w", pady=(8, 0))


class FootnotePanel:
    """Footnote styling and PDF footer options."""

    def __init__(
        self,
        parent: ttk.Frame,
        *,
        include_notes_var: tk.BooleanVar,
        footnote_size_var: tk.DoubleVar,
        footnote_text_color_var: tk.StringVar,
        footnote_marker_color_var: tk.StringVar,
        footer_color_var: tk.StringVar,
        footer_align_var: tk.StringVar,
        footer_text_var: tk.StringVar,
    ) -> None:
        box = ttk.LabelFrame(parent, text="Footnotes", padding=10)
        box.grid(row=1, column=1, sticky="nsew", pady=(0, 8), padx=(6, 0))
        self.frame = box

        ttk.Checkbutton(box, text="Enable footnotes", variable=include_notes_var).grid(
            row=0, column=0, columnspan=4, sticky="w"
        )
        ttk.Label(box, text="Size").grid(row=1, column=0, sticky="w", pady=(8, 0))
        ttk.Spinbox(
            box, from_=7, to=16, increment=0.5, textvariable=footnote_size_var, width=7
        ).grid(row=1, column=1, sticky="w", padx=(8, 12), pady=(8, 0))
        ttk.Label(box, text="Text color").grid(row=1, column=2, sticky="w", pady=(8, 0))
        ttk.Entry(box, textvariable=footnote_text_color_var, width=10).grid(
            row=1, column=3, sticky="w", padx=(8, 0), pady=(8, 0)
        )
        ttk.Label(box, text="Marker color").grid(
            row=2, column=0, sticky="w", pady=(8, 0)
        )
        ttk.Entry(box, textvariable=footnote_marker_color_var, width=10).grid(
            row=2, column=1, sticky="w", padx=(8, 12), pady=(8, 0)
        )

        ttk.Separator(box, orient=tk.HORIZONTAL).grid(
            row=3, column=0, columnspan=4, sticky="ew", pady=(10, 8)
        )
        ttk.Label(box, text="PDF pagination").grid(row=4, column=0, sticky="w")
        ttk.Label(box, text="Alignment").grid(row=4, column=1, sticky="w", padx=(8, 0))
        ttk.Combobox(
            box,
            textvariable=footer_align_var,
            values=("Right", "Center"),
            state="readonly",
            width=10,
        ).grid(row=4, column=2, sticky="w")
        ttk.Label(box, text="Footer color").grid(
            row=5, column=0, sticky="w", pady=(8, 0)
        )
        ttk.Entry(box, textvariable=footer_color_var, width=10).grid(
            row=5, column=1, sticky="w", padx=(8, 12), pady=(8, 0)
        )
        ttk.Label(box, text="Footer text").grid(
            row=6, column=0, sticky="w", pady=(8, 0)
        )
        ttk.Entry(box, textvariable=footer_text_var, width=30).grid(
            row=6, column=1, columnspan=3, sticky="ew", padx=(8, 0), pady=(8, 0)
        )
