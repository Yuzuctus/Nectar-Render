from __future__ import annotations

from types import SimpleNamespace

from nectar_render.ui.widgets import FontAutocomplete


class DummyVar:
    def __init__(self, value: str) -> None:
        self.value = value

    def get(self) -> str:
        return self.value

    def set(self, value: str) -> None:
        self.value = value


def test_focus_out_does_not_pollute_values_for_custom_entries() -> None:
    widget = SimpleNamespace(
        _allow_custom_values=True,
        var=DummyVar("JetBrains Mono Alt"),
        _all_values=["Consolas", "JetBrains Mono"],
        _last_valid_value="JetBrains Mono",
    )

    FontAutocomplete._on_focus_out(widget, None)

    assert widget.var.get() == "JetBrains Mono Alt"
    assert widget._all_values == ["Consolas", "JetBrains Mono"]


def test_focus_out_restores_last_valid_value_when_custom_values_disallowed() -> None:
    widget = SimpleNamespace(
        _allow_custom_values=False,
        var=DummyVar("Unknown Theme"),
        _all_values=["default", "monokai"],
        _last_valid_value="default",
    )

    FontAutocomplete._on_focus_out(widget, None)

    assert widget.var.get() == "default"
    assert widget._all_values == ["default", "monokai"]
