"""Tests for built-in presets."""

from nectar_render.ui.presets import (
    BUILTIN_PRESETS,
    BUILTIN_PRESET_NAMES,
    get_builtin_preset,
    is_builtin_preset,
)


# All style keys that a preset should define
_REQUIRED_STYLE_KEYS = {
    "body_font_var",
    "heading_font_var",
    "body_size_var",
    "line_height_var",
    "heading_color_var",
    "code_font_var",
    "code_size_var",
    "code_line_height_var",
    "code_theme_var",
    "margin_top_var",
    "margin_right_var",
    "margin_bottom_var",
    "margin_left_var",
    "footer_text_var",
    "footer_align_var",
    "include_notes_var",
    "footnote_size_var",
    "footnote_text_color_var",
    "footnote_marker_color_var",
    "table_stripes_var",
    "table_odd_color_var",
    "table_even_color_var",
    "table_pad_y_var",
    "table_pad_x_var",
    "image_scale_var",
    "show_horizontal_rules_var",
}


def test_five_builtin_presets_exist() -> None:
    assert len(BUILTIN_PRESETS) == 5


def test_builtin_preset_names_are_sorted() -> None:
    assert BUILTIN_PRESET_NAMES == sorted(BUILTIN_PRESETS.keys())


def test_all_presets_contain_required_keys() -> None:
    for name, preset in BUILTIN_PRESETS.items():
        missing = _REQUIRED_STYLE_KEYS - set(preset.keys())
        assert not missing, f"Preset '{name}' is missing keys: {missing}"


def test_presets_do_not_contain_file_path_keys() -> None:
    non_style_keys = {"markdown_var", "output_dir_var", "ui_theme_var", "auto_preview_var"}
    for name, preset in BUILTIN_PRESETS.items():
        found = non_style_keys & set(preset.keys())
        assert not found, f"Preset '{name}' should not contain non-style keys: {found}"


def test_font_sizes_are_reasonable() -> None:
    for name, preset in BUILTIN_PRESETS.items():
        assert 8 <= preset["body_size_var"] <= 24, f"{name}: body size out of range"
        assert 8 <= preset["code_size_var"] <= 24, f"{name}: code size out of range"


def test_margins_are_positive() -> None:
    for name, preset in BUILTIN_PRESETS.items():
        for key in ("margin_top_var", "margin_right_var", "margin_bottom_var", "margin_left_var"):
            assert preset[key] > 0, f"{name}: {key} must be positive"


def test_image_scale_is_valid_percentage() -> None:
    for name, preset in BUILTIN_PRESETS.items():
        scale = preset["image_scale_var"]
        assert 40 <= scale <= 100, f"{name}: image_scale_var {scale} out of 40-100 range"


def test_is_builtin_preset_true() -> None:
    assert is_builtin_preset("Modern")
    assert is_builtin_preset("Modern (built-in)")


def test_is_builtin_preset_false() -> None:
    assert not is_builtin_preset("My Custom Preset")


def test_get_builtin_preset_returns_copy() -> None:
    preset = get_builtin_preset("Academic")
    assert preset is not None
    assert preset is not BUILTIN_PRESETS["Academic"]
    assert preset == BUILTIN_PRESETS["Academic"]


def test_get_builtin_preset_none_for_unknown() -> None:
    assert get_builtin_preset("Nonexistent") is None


def test_get_builtin_preset_strips_suffix() -> None:
    preset = get_builtin_preset("Technical (built-in)")
    assert preset is not None
    assert preset["code_theme_var"] == "monokai"
