"""Tests for built-in presets."""

from dataclasses import fields

from nectar_render.core.presets import (
    BUILTIN_PRESET_NAMES,
    BUILTIN_PRESET_STYLES,
    get_builtin_preset,
    is_builtin_preset,
)
from nectar_render.core.styles import StyleOptions


def test_nine_builtin_presets_exist() -> None:
    assert len(BUILTIN_PRESET_STYLES) == 9


def test_builtin_preset_names_are_sorted() -> None:
    assert BUILTIN_PRESET_NAMES == sorted(BUILTIN_PRESET_STYLES.keys())


def test_all_presets_are_valid_style_options() -> None:
    required_fields = {f.name for f in fields(StyleOptions)}
    for name, preset in BUILTIN_PRESET_STYLES.items():
        missing = required_fields - set(f.name for f in fields(preset))
        assert not missing, f"Preset '{name}' is missing StyleOptions fields: {missing}"


def test_presets_have_valid_font_sizes() -> None:
    for name, preset in BUILTIN_PRESET_STYLES.items():
        assert 8 <= preset.body_font_size <= 24, f"{name}: body_font_size out of range"
        assert 8 <= preset.code_font_size <= 24, f"{name}: code_font_size out of range"


def test_presets_have_valid_margins() -> None:
    for name, preset in BUILTIN_PRESET_STYLES.items():
        assert 0.0 <= preset.margin_top_mm <= 100.0, (
            f"{name}: margin_top_mm out of range"
        )
        assert 0.0 <= preset.margin_right_mm <= 100.0, (
            f"{name}: margin_right_mm out of range"
        )
        assert 0.0 <= preset.margin_bottom_mm <= 100.0, (
            f"{name}: margin_bottom_mm out of range"
        )
        assert 0.0 <= preset.margin_left_mm <= 100.0, (
            f"{name}: margin_left_mm out of range"
        )


def test_presets_have_valid_image_scale() -> None:
    for name, preset in BUILTIN_PRESET_STYLES.items():
        assert 0.4 <= preset.image_scale <= 1.0, f"{name}: image_scale out of range"


def test_is_builtin_preset_true() -> None:
    assert is_builtin_preset("Academic")
    assert is_builtin_preset("Academic (built-in)")


def test_is_builtin_preset_false() -> None:
    assert not is_builtin_preset("My Custom Preset")
    assert not is_builtin_preset("Modern")
    assert not is_builtin_preset("Dark Code")


def test_get_builtin_preset_returns_copy() -> None:
    preset = get_builtin_preset("Academic")
    assert preset is not None
    assert preset is not BUILTIN_PRESET_STYLES["Academic"]
    assert preset == BUILTIN_PRESET_STYLES["Academic"]


def test_get_builtin_preset_none_for_unknown() -> None:
    assert get_builtin_preset("Nonexistent") is None


def test_get_builtin_preset_strips_suffix() -> None:
    preset = get_builtin_preset("Technical (built-in)")
    assert preset is not None
    assert preset.code_theme == "monokai"
