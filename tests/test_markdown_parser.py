import os
from pathlib import Path

import nectar_render.converter.markdown_parser as markdown_parser_module
from bs4 import BeautifulSoup
from nectar_render.config import CompressionOptions, ExportOptions, StyleOptions
from nectar_render.converter.footnotes import (
    extract_footnote_definitions,
    inject_paged_footnotes,
)
from nectar_render.converter.html_builder import build_document_html
from nectar_render.converter.markdown_parser import (
    invalidate_image_index_cache,
    normalize_obsidian_image_embeds,
    normalize_pagebreak_markers,
    parse_markdown,
)
from nectar_render.services.pdf_compression_service import PdfCompressionService
import nectar_render.utils.paths as paths_module
import nectar_render.utils.weasyprint_runtime as runtime_module


def test_pagebreak_marker_normalization() -> None:
    src = "A\n\n<!-- pagebreak -->\n\nB"
    normalized = normalize_pagebreak_markers(src)
    assert "page-break" in normalized


def test_footnotes_extraction() -> None:
    src = "Texte[^a]\n\n[^a]: note"
    res = extract_footnote_definitions(src)
    assert "a" in res.definitions
    assert res.definitions["a"] == "note"


def test_footnotes_injection_enabled() -> None:
    src = "Texte[^a]\n\n[^a]: note"
    injected = inject_paged_footnotes(src, enabled=True)
    assert 'class="footnote"' in injected


def test_footnotes_injection_adds_fallback_list() -> None:
    src = "Texte[^a]\n\n[^a]: note"
    injected = inject_paged_footnotes(src, enabled=True)
    assert 'class="footnote-ref"' in injected
    assert 'class="footnotes-list"' in injected
    assert 'id="fn-a"' in injected


def test_footnotes_injection_escapes_html() -> None:
    src = "Texte[^a]\n\n[^a]: <img src=x onerror=alert(1)>"
    injected = inject_paged_footnotes(src, enabled=True)
    assert "<img" not in injected
    assert "&lt;img src=x onerror=alert(1)&gt;" in injected


def test_pagebreak_markers_not_replaced_in_fenced_code() -> None:
    src = """Avant

```md
<!-- pagebreak -->
\\pagebreak
[[PAGEBREAK]]
```

Apres
"""
    normalized = normalize_pagebreak_markers(src)
    assert normalized.count('class="page-break"') == 0
    assert "<!-- pagebreak -->" in normalized


def test_pagination_css_includes_heuristics_and_manual_classes() -> None:
    html = build_document_html(
        "<h2>Titre</h2><p>Intro</p><ul><li>A</li></ul>", StyleOptions(), "A4"
    )
    assert "h2 + p" in html
    assert "p + ul" in html
    assert ".keep-with-next" in html
    assert ".keep-together" in html
    assert "counter(page)" in html
    assert "counter(pages)" in html


def test_footer_text_and_center_alignment_css_are_applied() -> None:
    html = build_document_html(
        "<p>Demo</p>",
        StyleOptions(footer_text="Confidentiel", footer_align="center"),
        "A4",
    )
    assert "@bottom-center" in html
    assert "Confidentiel" in html


def test_image_scale_css_is_applied() -> None:
    html = build_document_html(
        '<p><img src="x.png" /></p>', StyleOptions(image_scale=0.75), "A4"
    )
    assert "max-width: 75.0%" in html


def test_code_line_height_css_is_applied() -> None:
    html = build_document_html(
        "<pre><code>print(1)</code></pre>", StyleOptions(code_line_height=1.9), "A4"
    )
    assert "line-height: 1.9;" in html


def test_heading_h6_css_is_applied() -> None:
    html = build_document_html(
        "<h6>Note</h6>",
        StyleOptions(heading_h6_color="#123456", heading_h6_size_px=15),
        "A4",
    )
    assert "h6 { color: #123456; font-size: 15px; }" in html


def test_heading_before_image_block_uses_relaxed_break_rule() -> None:
    html = build_document_html(
        '<h2>Annexes</h2><p class="image-block"><img src="x.png"/></p>',
        StyleOptions(),
        "A4",
    )
    assert "h2 + p.image-block" in html
    assert "page-break-before: auto;" in html


def test_image_relative_path_is_resolved_to_file_uri(tmp_path: Path) -> None:
    image_path = tmp_path / "images" / "diagram.png"
    image_path.parent.mkdir(parents=True)
    image_path.write_bytes(b"fake")

    html = parse_markdown(
        "![Diag](images/diagram.png)", include_footnotes=False, assets_root=tmp_path
    )
    assert image_path.resolve().as_uri() in html


def test_image_filename_is_found_in_subfolders(tmp_path: Path) -> None:
    image_path = tmp_path / "assets" / "nested" / "schema.png"
    image_path.parent.mkdir(parents=True)
    image_path.write_bytes(b"fake")

    html = parse_markdown(
        "![Schema](schema.png)", include_footnotes=False, assets_root=tmp_path
    )
    assert image_path.resolve().as_uri() in html


def test_image_index_is_cached_for_same_assets_root(
    monkeypatch, tmp_path: Path
) -> None:
    image_path = tmp_path / "assets" / "nested" / "schema.png"
    image_path.parent.mkdir(parents=True)
    image_path.write_bytes(b"fake")

    invalidate_image_index_cache()
    build_calls: list[Path] = []
    original_builder = markdown_parser_module._build_image_index

    def _counting_builder(
        root: Path, file_names: set[str] | None = None
    ) -> dict[str, list[Path]]:
        build_calls.append(root)
        return original_builder(root, file_names)

    monkeypatch.setattr(markdown_parser_module, "_build_image_index", _counting_builder)

    parse_markdown(
        "![Schema](schema.png)", include_footnotes=False, assets_root=tmp_path
    )
    parse_markdown(
        "![Schema](schema.png)", include_footnotes=False, assets_root=tmp_path
    )

    assert build_calls == [tmp_path]


def test_image_index_cache_can_be_invalidated(monkeypatch, tmp_path: Path) -> None:
    image_path = tmp_path / "assets" / "nested" / "schema.png"
    image_path.parent.mkdir(parents=True)
    image_path.write_bytes(b"fake")

    invalidate_image_index_cache()
    build_calls: list[Path] = []
    original_builder = markdown_parser_module._build_image_index

    def _counting_builder(
        root: Path, file_names: set[str] | None = None
    ) -> dict[str, list[Path]]:
        build_calls.append(root)
        return original_builder(root, file_names)

    monkeypatch.setattr(markdown_parser_module, "_build_image_index", _counting_builder)

    parse_markdown(
        "![Schema](schema.png)", include_footnotes=False, assets_root=tmp_path
    )
    invalidate_image_index_cache(tmp_path)
    parse_markdown(
        "![Schema](schema.png)", include_footnotes=False, assets_root=tmp_path
    )

    assert build_calls == [tmp_path, tmp_path]


def test_direct_image_resolution_skips_index_fallback(
    monkeypatch, tmp_path: Path
) -> None:
    image_path = tmp_path / "images" / "diagram.png"
    image_path.parent.mkdir(parents=True)
    image_path.write_bytes(b"fake")

    def fail_get_image_index(root: Path, file_names: set[str]) -> dict[str, list[Path]]:
        raise AssertionError("image index fallback should not run for direct hits")

    monkeypatch.setattr(
        markdown_parser_module, "_get_image_index", fail_get_image_index
    )

    html = parse_markdown(
        "![Diag](images/diagram.png)", include_footnotes=False, assets_root=tmp_path
    )

    assert image_path.resolve().as_uri() in html


def test_image_index_cache_is_bounded_by_root_count(tmp_path: Path) -> None:
    invalidate_image_index_cache()

    for index in range(markdown_parser_module._IMAGE_CACHE_MAX_ROOTS + 3):
        assets_root = tmp_path / f"root-{index}"
        image_path = assets_root / "nested" / "schema.png"
        image_path.parent.mkdir(parents=True)
        image_path.write_bytes(b"fake")
        parse_markdown(
            "![Schema](schema.png)", include_footnotes=False, assets_root=assets_root
        )

    assert (
        len(markdown_parser_module._IMAGE_INDEX_CACHE)
        <= markdown_parser_module._IMAGE_CACHE_MAX_ROOTS
    )


def test_image_url_encoded_path_and_query_are_resolved(tmp_path: Path) -> None:
    image_path = tmp_path / "dossier images" / "plan final.png"
    image_path.parent.mkdir(parents=True)
    image_path.write_bytes(b"fake")

    html = parse_markdown(
        "![Plan](dossier%20images/plan%20final.png?v=2#section)",
        include_footnotes=False,
        assets_root=tmp_path,
    )
    assert image_path.resolve().as_uri() in html


def test_obsidian_image_embed_is_converted_and_resolved(tmp_path: Path) -> None:
    image_path = tmp_path / "Snipaste_2026-02-09_14-51-56.png"
    image_path.write_bytes(b"fake")

    html = parse_markdown(
        "![[Snipaste_2026-02-09_14-51-56.png]]",
        include_footnotes=False,
        assets_root=tmp_path,
    )
    assert image_path.resolve().as_uri() in html


def test_obsidian_adjacent_image_embeds_are_all_converted() -> None:
    normalized = normalize_obsidian_image_embeds("![[a.png]]![[b.png]]")
    assert normalized == "![](a.png)![](b.png)"


def test_consecutive_images_are_split_into_image_blocks(tmp_path: Path) -> None:
    image_a = tmp_path / "a.png"
    image_b = tmp_path / "b.png"
    image_a.write_bytes(b"a")
    image_b.write_bytes(b"b")

    html = parse_markdown(
        "## Annexes\n\n![[a.png]]\n![[b.png]]",
        include_footnotes=False,
        assets_root=tmp_path,
    )
    soup = BeautifulSoup(html, "html.parser")
    image_blocks = soup.select("p.image-block")
    assert len(image_blocks) >= 2


def test_parse_markdown_sanitizes_html_by_default() -> None:
    html = parse_markdown(
        'Avant<script>alert(1)</script><p onclick="x()">ok</p>',
        include_footnotes=False,
    )
    assert "<script" not in html
    assert "onclick=" not in html


def test_parse_markdown_keeps_raw_html_when_sanitization_disabled() -> None:
    html = parse_markdown(
        "Avant<script>alert(1)</script>",
        include_footnotes=False,
        sanitize_html=False,
    )
    assert "<script>alert(1)</script>" in html


def test_parse_markdown_neutralizes_file_links_when_sanitization_enabled() -> None:
    html = parse_markdown(
        '<a href="file:///C:/secret.txt">x</a>',
        include_footnotes=False,
        sanitize_html=True,
    )
    assert 'href="file:///C:/secret.txt"' not in html
    assert "<a>x</a>" in html


def test_parse_markdown_neutralizes_data_images_when_sanitization_enabled() -> None:
    html = parse_markdown(
        '<img src="data:image/png;base64,AAAA" alt="x"/>',
        include_footnotes=False,
        sanitize_html=True,
    )
    assert 'src="data:image/png;base64,AAAA"' not in html
    assert '<img alt="x"/>' in html or '<img alt="x">' in html


def test_heading_chain_gets_pagination_anchor_classes() -> None:
    html = parse_markdown(
        "# Titre 1\n\n## Titre 2\n\nTexte d'introduction.",
        include_footnotes=False,
    )
    soup = BeautifulSoup(html, "html.parser")

    h1 = soup.find("h1")
    h2 = soup.find("h2")
    paragraph = soup.find("p")

    assert h1 is not None
    assert h2 is not None
    assert paragraph is not None
    assert "keep-with-next" in h1.get("class", [])
    assert "keep-with-next" in h2.get("class", [])
    assert "keep-with-prev" in h2.get("class", [])
    assert "keep-with-prev" in paragraph.get("class", [])


def test_short_list_after_intro_is_marked_compact_and_kept_together() -> None:
    html = parse_markdown(
        "Liste a puce :\n\n- nanan\n- nanana",
        include_footnotes=False,
    )
    soup = BeautifulSoup(html, "html.parser")
    bullet_list = soup.find("ul")

    assert bullet_list is not None
    classes = set(bullet_list.get("class", []))
    assert {"compact-list", "keep-together", "keep-with-prev"} <= classes


def test_long_list_is_not_forced_into_compact_mode() -> None:
    markdown_text = "\n".join(
        ["Liste longue :", ""] + [f"- element {index}" for index in range(1, 9)]
    )
    html = parse_markdown(markdown_text, include_footnotes=False)
    soup = BeautifulSoup(html, "html.parser")
    bullet_list = soup.find("ul")

    assert bullet_list is not None
    classes = set(bullet_list.get("class", []))
    assert "compact-list" not in classes
    assert "keep-together" not in classes


def test_export_options_enable_pdf_compression_by_default() -> None:
    export = ExportOptions()
    assert isinstance(export.compression, CompressionOptions)
    assert export.compression.enabled is True
    assert export.compression.profile == "balanced"
    assert export.compression.remove_metadata is True


def test_pdf_compression_service_disabled_is_safe_noop(tmp_path: Path) -> None:
    pdf_path = tmp_path / "demo.pdf"
    pdf_path.write_bytes(b"fake-pdf-content")

    service = PdfCompressionService()
    result = service.compress(pdf_path, CompressionOptions(enabled=False))

    assert result.path == pdf_path
    assert result.applied is False
    assert result.original_size == result.final_size
    assert pdf_path.read_bytes() == b"fake-pdf-content"


def test_pdf_compression_service_without_qpdf_keeps_pdf(tmp_path: Path) -> None:
    pdf_path = tmp_path / "demo.pdf"
    pdf_path.write_bytes(b"fake-pdf-content")

    service = PdfCompressionService()
    options = CompressionOptions(
        enabled=True, profile="balanced", remove_metadata=False
    )
    result = service.compress(pdf_path, options)

    assert result.path == pdf_path
    assert result.final_size == result.original_size
    assert pdf_path.exists()


def test_application_data_dir_uses_appdata_on_windows(
    monkeypatch, tmp_path: Path
) -> None:
    monkeypatch.setattr(paths_module.sys, "platform", "win32")
    monkeypatch.setenv("APPDATA", str(tmp_path))

    assert paths_module.application_data_dir() == tmp_path / "nectar-render"


def test_prepare_weasyprint_environment_uses_existing_msys2_dirs(
    monkeypatch, tmp_path: Path
) -> None:
    msys2_root = tmp_path / "msys64"
    ucrt_dir = msys2_root / "ucrt64" / "bin"
    mingw_dir = msys2_root / "mingw64" / "bin"
    ucrt_dir.mkdir(parents=True)
    mingw_dir.mkdir(parents=True)

    added_directories: list[str] = []

    monkeypatch.setattr(runtime_module.sys, "platform", "win32")
    monkeypatch.setenv("MSYS2_ROOT", str(msys2_root))
    monkeypatch.delenv("WEASYPRINT_DLL_DIRECTORIES", raising=False)
    monkeypatch.setattr(
        runtime_module.os,
        "add_dll_directory",
        lambda path: added_directories.append(path) or object(),
        raising=False,
    )

    runtime_module._DLL_DIRECTORY_HANDLES.clear()
    runtime_module._REGISTERED_DLL_DIRECTORIES.clear()

    status = runtime_module.prepare_weasyprint_environment()

    assert ucrt_dir in status.configured_directories
    assert mingw_dir in status.configured_directories
    assert os.environ["WEASYPRINT_DLL_DIRECTORIES"].startswith(
        f"{ucrt_dir};{mingw_dir}"
    )
    assert str(ucrt_dir) in added_directories
    assert str(mingw_dir) in added_directories


def test_prepare_weasyprint_environment_uses_runtime_status_cache(monkeypatch) -> None:
    calls: list[str] = []

    monkeypatch.setattr(runtime_module.sys, "platform", "linux")
    monkeypatch.setattr(
        runtime_module,
        "_candidate_runtime_directories",
        lambda: calls.append("scan") or [],
    )
    runtime_module._RUNTIME_STATUS_CACHE.clear()

    first = runtime_module.prepare_weasyprint_environment()
    second = runtime_module.prepare_weasyprint_environment()

    assert first == second
    assert calls == ["scan"]


def test_prepare_weasyprint_environment_force_refresh_bypasses_cache(
    monkeypatch,
) -> None:
    calls: list[str] = []

    monkeypatch.setattr(runtime_module.sys, "platform", "linux")
    monkeypatch.setattr(
        runtime_module,
        "_candidate_runtime_directories",
        lambda: calls.append("scan") or [],
    )
    runtime_module._RUNTIME_STATUS_CACHE.clear()

    runtime_module.prepare_weasyprint_environment()
    runtime_module.prepare_weasyprint_environment(force_refresh=True)

    assert calls == ["scan", "scan"]


def test_build_windows_runtime_help_mentions_manual_fix() -> None:
    message = runtime_module.build_windows_runtime_help(
        OSError("cannot load library 'libgobject-2.0-0'"),
        runtime_module.WeasyPrintRuntimeStatus(
            configured_directories=(), searched_directories=()
        ),
    )

    assert "MSYS2.MSYS2" in message
    assert "mingw-w64-ucrt-x86_64-pango" in message
    assert "HTML export remains available" in message
