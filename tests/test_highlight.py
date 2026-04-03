"""Tests for the code highlighting module."""

from nectar_render.converter.highlight import (
    build_pygments_css,
    list_available_styles,
    resolve_style_name,
)


def test_list_available_styles_returns_known_themes() -> None:
    styles = list_available_styles()
    assert isinstance(styles, list)
    assert len(styles) > 0
    assert "default" in styles
    assert "monokai" in styles


def test_resolve_style_name_maps_light_alias() -> None:
    assert resolve_style_name("light") == "default"


def test_resolve_style_name_maps_dark_alias() -> None:
    assert resolve_style_name("dark") == "native"


def test_resolve_style_name_passes_through_known_name() -> None:
    assert resolve_style_name("monokai") == "monokai"


def test_resolve_style_name_passes_through_unknown() -> None:
    result = resolve_style_name("nonexistent_theme_xyz")
    assert result == "nonexistent_theme_xyz"


def test_build_pygments_css_returns_codehilite_css() -> None:
    css = build_pygments_css("default")
    assert ".codehilite" in css
    assert "color:" in css.lower() or "background" in css.lower()


def test_build_pygments_css_monokai_differs_from_default() -> None:
    default_css = build_pygments_css("default")
    monokai_css = build_pygments_css("monokai")
    assert default_css != monokai_css


def test_build_pygments_css_falls_back_to_default_for_unknown_style() -> None:
    assert build_pygments_css("nonexistent_theme_xyz") == build_pygments_css("default")
