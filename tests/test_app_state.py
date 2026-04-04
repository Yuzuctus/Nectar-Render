from __future__ import annotations

from nectar_render.interfaces.desktop.app_state import (
    collect_export_options,
    detect_markdown_max_heading_level_from_text,
)


def test_collect_export_options_normalizes_invalid_profile() -> None:
    export = collect_export_options(
        output_format="pdf+html",
        page_size="letter",
        compression_enabled=True,
        compression_profile="ultra",
        remove_metadata=False,
    )

    assert export.output_format == "pdf+html"
    assert export.page_size == "letter"
    assert export.compression.enabled is True
    assert export.compression.profile == "balanced"
    assert export.compression.remove_metadata is False


def test_detect_markdown_max_heading_level_ignores_fenced_code() -> None:
    markdown_text = """# Visible

```md
###### Hidden
```

### Still visible
"""
    assert detect_markdown_max_heading_level_from_text(markdown_text) == 3
