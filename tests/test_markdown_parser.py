import os
from pathlib import Path

from md_to_pdf.config import CompressionOptions, ExportOptions, StyleOptions
from md_to_pdf.converter.footnotes import extract_footnote_definitions, inject_paged_footnotes
from md_to_pdf.converter.html_builder import build_document_html
from md_to_pdf.converter.markdown_parser import normalize_obsidian_image_embeds, normalize_pagebreak_markers, parse_markdown
from md_to_pdf.services.pdf_compression_service import PdfCompressionService
import md_to_pdf.utils.paths as paths_module
import md_to_pdf.utils.weasyprint_runtime as runtime_module


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
    assert "class=\"footnote\"" in injected


def test_footnotes_injection_adds_fallback_list() -> None:
    src = "Texte[^a]\n\n[^a]: note"
    injected = inject_paged_footnotes(src, enabled=True)
    assert "class=\"footnote-ref\"" in injected
    assert "class=\"footnotes-list\"" in injected
    assert "id=\"fn-a\"" in injected


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
    assert normalized.count("class=\"page-break\"") == 0
    assert "<!-- pagebreak -->" in normalized


def test_pagination_css_includes_heuristics_and_manual_classes() -> None:
    html = build_document_html("<h2>Titre</h2><p>Intro</p><ul><li>A</li></ul>", StyleOptions(), "A4")
    assert "h2 + p" in html
    assert "p + ul" in html
    assert ".keep-with-next" in html
    assert ".keep-together" in html
    assert "counter(page)" in html
    assert "counter(pages)" in html


def test_footer_text_and_center_alignment_css_are_applied() -> None:
    html = build_document_html("<p>Demo</p>", StyleOptions(footer_text="Confidentiel", footer_align="center"), "A4")
    assert "@bottom-center" in html
    assert "Confidentiel" in html


def test_image_scale_css_is_applied() -> None:
    html = build_document_html("<p><img src=\"x.png\" /></p>", StyleOptions(image_scale=0.75), "A4")
    assert "max-width: 75.0%" in html


def test_code_line_height_css_is_applied() -> None:
    html = build_document_html("<pre><code>print(1)</code></pre>", StyleOptions(code_line_height=1.9), "A4")
    assert "line-height: 1.9;" in html


def test_heading_h6_css_is_applied() -> None:
    html = build_document_html("<h6>Note</h6>", StyleOptions(heading_h6_color="#123456", heading_h6_size_px=15), "A4")
    assert "h6 { color: #123456; font-size: 15px; }" in html


def test_heading_before_image_block_uses_relaxed_break_rule() -> None:
    html = build_document_html("<h2>Annexes</h2><p class=\"image-block\"><img src=\"x.png\"/></p>", StyleOptions(), "A4")
    assert "h2 + p.image-block" in html
    assert "page-break-before: auto;" in html


def test_image_relative_path_is_resolved_to_file_uri(tmp_path: Path) -> None:
    image_path = tmp_path / "images" / "diagram.png"
    image_path.parent.mkdir(parents=True)
    image_path.write_bytes(b"fake")

    html = parse_markdown("![Diag](images/diagram.png)", include_footnotes=False, assets_root=tmp_path)
    assert image_path.resolve().as_uri() in html


def test_image_filename_is_found_in_subfolders(tmp_path: Path) -> None:
    image_path = tmp_path / "assets" / "nested" / "schema.png"
    image_path.parent.mkdir(parents=True)
    image_path.write_bytes(b"fake")

    html = parse_markdown("![Schema](schema.png)", include_footnotes=False, assets_root=tmp_path)
    assert image_path.resolve().as_uri() in html


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

    html = parse_markdown("## Annexes\n\n![[a.png]]\n![[b.png]]", include_footnotes=False, assets_root=tmp_path)
    assert html.count('class="image-block"') >= 2


def test_parse_markdown_can_sanitize_html_when_enabled() -> None:
    html = parse_markdown(
        "Avant<script>alert(1)</script><p onclick=\"x()\">ok</p>",
        include_footnotes=False,
        sanitize_html=True,
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
    options = CompressionOptions(enabled=True, profile="balanced", remove_metadata=False)
    result = service.compress(pdf_path, options)

    assert result.path == pdf_path
    assert result.final_size == result.original_size
    assert pdf_path.exists()


def test_application_data_dir_uses_appdata_on_windows(monkeypatch, tmp_path: Path) -> None:
    monkeypatch.setattr(paths_module.sys, "platform", "win32")
    monkeypatch.setenv("APPDATA", str(tmp_path))

    assert paths_module.application_data_dir() == tmp_path / "md-to-pdf"


def test_prepare_weasyprint_environment_uses_existing_msys2_dirs(monkeypatch, tmp_path: Path) -> None:
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
    assert os.environ["WEASYPRINT_DLL_DIRECTORIES"].startswith(f"{ucrt_dir};{mingw_dir}")
    assert str(ucrt_dir) in added_directories
    assert str(mingw_dir) in added_directories


def test_build_windows_runtime_help_mentions_manual_fix() -> None:
    message = runtime_module.build_windows_runtime_help(
        OSError("cannot load library 'libgobject-2.0-0'"),
        runtime_module.WeasyPrintRuntimeStatus(configured_directories=(), searched_directories=()),
    )

    assert "MSYS2.MSYS2" in message
    assert "mingw-w64-ucrt-x86_64-pango" in message
    assert "HTML export remains available" in message
