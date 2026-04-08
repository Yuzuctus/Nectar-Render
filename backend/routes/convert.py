from __future__ import annotations

import asyncio
import logging
import re
import shutil
import tempfile
import zipfile
from collections.abc import Mapping
from functools import partial
from pathlib import Path
from typing import Annotated, Any
from urllib.parse import unquote, urlparse

from fastapi import APIRouter, File, Form, HTTPException, Request, UploadFile
from fastapi.responses import FileResponse, JSONResponse, Response
from starlette.background import BackgroundTask

from nectar_render.application.conversion import (
    ConversionService,
    ImageMode,
    analyze_markdown,
)
from nectar_render.core.presets import get_builtin_preset
from nectar_render.core.styles import (
    CompressionOptions,
    ExportOptions,
    StyleOptions,
    style_from_option_mapping,
)
from nectar_render.utils.paths import is_external_or_absolute_path

router = APIRouter(prefix="/analyze", tags=["conversion"])
convert_router = APIRouter(tags=["conversion"])

_MAX_MARKDOWN_BYTES = 2 * 1024 * 1024
_MAX_ASSET_COUNT = 128
_MAX_ASSET_BYTES = 10 * 1024 * 1024
_MAX_ASSETS_TOTAL_BYTES = 50 * 1024 * 1024
_CONVERSION_TIMEOUT_SECONDS = 120.0
_OUTPUT_FORMATS = {"PDF", "HTML", "PDF+HTML"}
_PAGE_SIZE_MAP = {
    "A4": "A4",
    "LETTER": "Letter",
    "LEGAL": "Legal",
    "A3": "A3",
    "A5": "A5",
}
_IMAGE_MODE_MAP = {
    "WITH_IMAGES": ImageMode.WITH_IMAGES,
    "ALT_ONLY": ImageMode.ALT_ONLY,
    "STRIP": ImageMode.STRIP,
}
_ALLOWED_IMAGE_EXTENSIONS = {
    ".apng",
    ".avif",
    ".bmp",
    ".gif",
    ".jpeg",
    ".jpg",
    ".png",
    ".svg",
    ".tif",
    ".tiff",
    ".webp",
}
_STYLE_FORM_FIELD_NAMES = (
    "body_font",
    "body_font_size",
    "line_height",
    "heading_font",
    "heading_color",
    "heading_h1_color",
    "heading_h2_color",
    "heading_h3_color",
    "heading_h4_color",
    "heading_h5_color",
    "heading_h6_color",
    "heading_h1_size_px",
    "heading_h2_size_px",
    "heading_h3_size_px",
    "heading_h4_size_px",
    "heading_h5_size_px",
    "heading_h6_size_px",
    "code_font",
    "code_font_size",
    "code_line_height",
    "code_theme",
    "margin_top_mm",
    "margin_right_mm",
    "margin_bottom_mm",
    "margin_left_mm",
    "footer_text",
    "footer_align",
    "footer_color",
    "include_footnotes",
    "footnote_font_size",
    "footnote_text_color",
    "footnote_marker_color",
    "table_row_stripes",
    "table_row_odd_color",
    "table_row_even_color",
    "table_cell_padding_y_px",
    "table_cell_padding_x_px",
    "image_scale",
    "sanitize_html",
    "show_horizontal_rules",
)
_STANDARD_IMAGE_RE = re.compile(
    r"!\[[^\]]*\]\((<[^>]+>|[^)\s]+)(?:\s+(?:\"[^\"]*\"|'[^']*'|\([^)]+\)))?\)",
    re.IGNORECASE,
)
_OBSIDIAN_IMAGE_RE = re.compile(r"!\[\[([^\]|]+)(?:\|[^\]]*)?\]\]")
_HTML_IMAGE_RE = re.compile(
    r"<img\b[^>]*\bsrc\s*=\s*(?:\"([^\"]+)\"|'([^']+)'|([^\s>]+))",
    re.IGNORECASE,
)


def _cleanup_task(tmp_dir: str) -> BackgroundTask:
    return BackgroundTask(shutil.rmtree, tmp_dir, ignore_errors=True)


def _parse_bool(value: str, default: bool) -> bool:
    cleaned = (value or "").strip().lower()
    if cleaned == "":
        return default
    return cleaned not in {"0", "false", "no", "off"}


def _safe_asset_filename(raw_name: str) -> str:
    sanitized = (raw_name or "").replace("\x00", "").strip()
    candidate = Path(sanitized).name
    if not candidate or candidate in {".", ".."}:
        raise HTTPException(status_code=400, detail="Invalid asset filename")
    return candidate


def _style_from_form_values(
    values: Mapping[str, Any],
    *,
    preset_name: str,
) -> StyleOptions:
    base_style: StyleOptions | None = None
    if preset_name:
        base_style = get_builtin_preset(preset_name)
        if base_style is None:
            raise HTTPException(
                status_code=400, detail=f"Unknown preset '{preset_name}'"
            )

    style_mapping = {
        key: value
        for key, value in values.items()
        if key in _STYLE_FORM_FIELD_NAMES and value != ""
    }
    return style_from_option_mapping(style_mapping, base_style=base_style)


def _export_from_form_values(values: Mapping[str, Any]) -> ExportOptions:
    output_format_raw = str(values.get("output_format", "PDF"))
    output_format_upper = output_format_raw.upper()
    if output_format_upper not in _OUTPUT_FORMATS:
        raise HTTPException(
            status_code=400,
            detail="Invalid output_format. Must be PDF, HTML, or PDF+HTML.",
        )

    page_size_raw = str(values.get("page_size", "A4"))
    page_size_upper = page_size_raw.upper()
    page_size_value = _PAGE_SIZE_MAP.get(page_size_upper)
    if page_size_value is None:
        raise HTTPException(
            status_code=400,
            detail="Invalid page_size. Must be A4, Letter, Legal, A3, or A5.",
        )

    compression_profile_raw = str(values.get("compression_profile", "balanced"))
    profile_value = compression_profile_raw.lower().strip() or "balanced"
    if profile_value not in {"balanced", "max"}:
        profile_value = "balanced"

    compression_timeout_raw = values.get("compression_timeout", "")
    try:
        compression_timeout = int(str(compression_timeout_raw).strip())
    except (TypeError, ValueError):
        compression_timeout = 45
    compression_timeout = max(5, compression_timeout)

    keep_original_raw = str(values.get("keep_original_on_fail", "true"))
    compress_pdf_raw = str(values.get("compress_pdf", ""))
    strip_metadata_raw = str(values.get("strip_metadata", ""))

    return ExportOptions(
        output_format=output_format_upper,
        page_size=page_size_value,
        compression=CompressionOptions(
            enabled=_parse_bool(compress_pdf_raw, True),
            profile=profile_value,
            remove_metadata=_parse_bool(strip_metadata_raw, True),
            timeout_sec=compression_timeout,
            keep_original_on_fail=_parse_bool(keep_original_raw, True),
        ),
    )


def _image_mode_from_value(image_mode: str) -> ImageMode:
    image_mode_upper = image_mode.upper()
    image_mode_value = _IMAGE_MODE_MAP.get(image_mode_upper)
    if image_mode_value is None:
        raise HTTPException(
            status_code=400,
            detail="Invalid image_mode. Must be WITH_IMAGES, ALT_ONLY, or STRIP.",
        )
    return image_mode_value


def _collect_referenced_images(markdown_text: str) -> list[str]:
    all_refs: list[str] = []

    for match in _STANDARD_IMAGE_RE.finditer(markdown_text):
        ref = match.group(1).strip()
        if not ref:
            continue
        if _is_inside_code_block(markdown_text, match.start()):
            continue
        if _is_inside_inline_code(markdown_text, match.start(), match.end()):
            continue
        if ref.startswith("<") and ref.endswith(">") and len(ref) > 2:
            ref = ref[1:-1].strip()
        if not ref:
            continue
        all_refs.append(ref)

    for match in _OBSIDIAN_IMAGE_RE.finditer(markdown_text):
        ref = match.group(1).strip()
        if not ref:
            continue
        if _is_inside_code_block(markdown_text, match.start()):
            continue
        if _is_inside_inline_code(markdown_text, match.start(), match.end()):
            continue
        all_refs.append(ref)

    for match in _HTML_IMAGE_RE.finditer(markdown_text):
        ref = (
            (match.group(1) or "").strip()
            or (match.group(2) or "").strip()
            or (match.group(3) or "").strip()
        )
        if not ref:
            continue
        if _is_inside_code_block(markdown_text, match.start()):
            continue
        if _is_inside_inline_code(markdown_text, match.start(), match.end()):
            continue
        all_refs.append(ref)

    return all_refs


def _validate_uploaded_assets_against_markdown(
    markdown_text: str,
    uploads: list[UploadFile] | None,
) -> list[str]:
    all_refs = _collect_referenced_images(markdown_text)

    uploaded_names: set[str] = set()
    for asset in uploads or []:
        if asset.filename:
            uploaded_names.add(_normalize_image_ref(asset.filename))

    missing: list[str] = []
    seen_missing: set[str] = set()
    rejected_refs: list[str] = []

    for ref in all_refs:
        if ref.lower().startswith("data:"):
            continue
        if is_external_or_absolute_path(ref):
            rejected_refs.append(ref)
            continue

        normalized_ref = _normalize_image_ref(ref)
        if normalized_ref in uploaded_names or normalized_ref in seen_missing:
            continue
        seen_missing.add(normalized_ref)
        missing.append(ref)

    if rejected_refs:
        raise HTTPException(
            status_code=400,
            detail=f"External URLs and absolute paths are not allowed: {rejected_refs[:5]}",
        )

    return missing


def _normalize_image_ref(ref: str) -> str:
    """Normalize an image reference for comparison.

    - Decode URL-encoded characters (%20 -> space)
    - Remove query strings (?v=1.0)
    - Remove fragment identifiers (#section)
    - Normalize path separators
    - Extract filename only
    """
    # Decode URL encoding
    decoded = unquote(ref).strip()
    # Parse and remove query/fragment
    parsed = urlparse(decoded)
    path_only = parsed.path or decoded
    # Normalize separators and get filename
    normalized = path_only.replace("\\", "/")
    filename = normalized.split("/")[-1]
    return filename


def _is_inside_code_block(markdown_text: str, match_start: int) -> bool:
    """Check if a match position is inside a fenced code block."""
    # Find all fenced code blocks (``` or ~~~)
    fence_pattern = re.compile(r"^(`{3,}|~{3,}).*$", re.MULTILINE)
    in_fence = False
    fence_char = ""

    for fence_match in fence_pattern.finditer(markdown_text):
        fence_start = fence_match.start()
        if fence_start > match_start:
            break
        fence = fence_match.group(1)
        if not in_fence:
            in_fence = True
            fence_char = fence[0]
        elif fence[0] == fence_char and len(fence) >= 3:
            in_fence = False
            fence_char = ""

    return in_fence


def _is_inside_inline_code(
    markdown_text: str, match_start: int, match_end: int
) -> bool:
    """Check if a match is inside inline code (backticks).

    Handles single, double, and triple backtick inline code spans correctly.
    Per CommonMark spec: a code span begins with a backtick string and ends
    with a backtick string of equal length.
    """
    # Get the line containing the match
    line_start = markdown_text.rfind("\n", 0, match_start) + 1
    line_end = markdown_text.find("\n", match_end)
    if line_end == -1:
        line_end = len(markdown_text)
    line = markdown_text[line_start:line_end]
    match_in_line_start = match_start - line_start

    # Parse inline code spans and check if position falls inside one
    i = 0
    while i < len(line):
        if line[i] == "`":
            # Count opening backticks
            open_start = i
            open_ticks = 0
            while i < len(line) and line[i] == "`":
                open_ticks += 1
                i += 1
            # Find matching closing backticks (same length)
            close_pos = -1
            j = i
            while j < len(line):
                if line[j] == "`":
                    close_start = j
                    close_ticks = 0
                    while j < len(line) and line[j] == "`":
                        close_ticks += 1
                        j += 1
                    if close_ticks == open_ticks:
                        close_pos = close_start
                        break
                else:
                    j += 1
            if close_pos != -1:
                # Check if match position is inside this code span
                code_start = open_start + open_ticks
                code_end = close_pos
                if code_start <= match_in_line_start < code_end:
                    return True
                i = close_pos + open_ticks
            # If no closing found, remaining line is not code (unclosed span)
        else:
            i += 1

    return False


def _validate_markdown_text(markdown_text: str) -> None:
    if not markdown_text.strip():
        raise HTTPException(status_code=400, detail="markdown_text cannot be empty")
    if len(markdown_text.encode("utf-8")) > _MAX_MARKDOWN_BYTES:
        raise HTTPException(
            status_code=413,
            detail=f"markdown_text is too large (max {_MAX_MARKDOWN_BYTES} bytes)",
        )


async def _read_markdown_upload(upload: UploadFile) -> str:
    content = await upload.read(_MAX_MARKDOWN_BYTES + 1)
    if len(content) > _MAX_MARKDOWN_BYTES:
        raise HTTPException(
            status_code=413,
            detail=f"Markdown file is too large (max {_MAX_MARKDOWN_BYTES} bytes)",
        )
    try:
        return content.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise HTTPException(
            status_code=400, detail="Markdown file must be UTF-8"
        ) from exc


async def _load_assets(
    uploads: list[UploadFile] | None,
    *,
    tmp_path: Path,
) -> set[str]:
    if not uploads:
        return set()

    if len(uploads) > _MAX_ASSET_COUNT:
        raise HTTPException(
            status_code=413,
            detail=f"Too many assets (max {_MAX_ASSET_COUNT})",
        )

    asset_names: set[str] = set()
    total_bytes = 0
    tmp_root = tmp_path.resolve()

    for asset in uploads:
        if not asset.filename:
            continue

        filename = _safe_asset_filename(asset.filename)
        extension = Path(filename).suffix.lower()
        if extension not in _ALLOWED_IMAGE_EXTENSIONS:
            raise HTTPException(
                status_code=400,
                detail=f"Unsupported asset type for '{filename}'",
            )

        if filename in asset_names:
            raise HTTPException(
                status_code=400,
                detail=f"Duplicate asset filename '{filename}'",
            )

        data = await asset.read(_MAX_ASSET_BYTES + 1)
        if len(data) > _MAX_ASSET_BYTES:
            raise HTTPException(
                status_code=413,
                detail=f"Asset '{filename}' exceeds {_MAX_ASSET_BYTES} bytes",
            )

        total_bytes += len(data)
        if total_bytes > _MAX_ASSETS_TOTAL_BYTES:
            raise HTTPException(
                status_code=413,
                detail=(f"Total asset payload exceeds {_MAX_ASSETS_TOTAL_BYTES} bytes"),
            )

        target = (tmp_path / filename).resolve()
        if target.parent != tmp_root:
            raise HTTPException(status_code=400, detail="Invalid asset path")

        target.write_bytes(data)
        asset_names.add(filename)

    return asset_names


def _extract_uploads(values: list[object]) -> list[UploadFile]:
    uploads: list[UploadFile] = []
    for value in values:
        if hasattr(value, "filename") and hasattr(value, "read"):
            uploads.append(value)  # type: ignore[arg-type]
    return uploads


@router.get("/health")
async def health() -> dict[str, str]:
    return {"status": "ok"}


@router.post("/")
async def analyze(
    file: Annotated[UploadFile, File(description="Markdown file")],
) -> dict[str, list[str]]:
    if not file.filename or Path(file.filename).suffix.lower() != ".md":
        raise HTTPException(status_code=400, detail="Only .md files are accepted")

    markdown_text = await _read_markdown_upload(file)
    missing = analyze_markdown(markdown_text)
    return {"missing_images": missing}


@convert_router.post(
    "/convert",
    responses={
        422: {"description": "Missing images"},
        504: {"description": "Conversion timeout"},
        500: {"description": "Conversion error"},
    },
)
async def convert(
    markdown_text: Annotated[str, Form(description="Raw markdown content")],
    image_mode: Annotated[
        str, Form(description="WITH_IMAGES | ALT_ONLY | STRIP")
    ] = "WITH_IMAGES",
    output_format: Annotated[str, Form(description="PDF | HTML | PDF+HTML")] = "PDF",
    page_size: Annotated[str, Form(description="A4 | Letter | Legal | A3 | A5")] = "A4",
    preset: Annotated[str, Form(description="Preset name (optional)")] = "",
    assets: Annotated[
        list[UploadFile] | None,
        File(description="Image files for WITH_IMAGES mode"),
    ] = None,
    body_font: Annotated[str, Form(description="")] = "",
    body_font_size: Annotated[str, Form(description="")] = "",
    line_height: Annotated[str, Form(description="")] = "",
    heading_font: Annotated[str, Form(description="")] = "",
    heading_color: Annotated[str, Form(description="")] = "",
    heading_h1_color: Annotated[str, Form(description="")] = "",
    heading_h2_color: Annotated[str, Form(description="")] = "",
    heading_h3_color: Annotated[str, Form(description="")] = "",
    heading_h4_color: Annotated[str, Form(description="")] = "",
    heading_h5_color: Annotated[str, Form(description="")] = "",
    heading_h6_color: Annotated[str, Form(description="")] = "",
    heading_h1_size_px: Annotated[str, Form(description="")] = "",
    heading_h2_size_px: Annotated[str, Form(description="")] = "",
    heading_h3_size_px: Annotated[str, Form(description="")] = "",
    heading_h4_size_px: Annotated[str, Form(description="")] = "",
    heading_h5_size_px: Annotated[str, Form(description="")] = "",
    heading_h6_size_px: Annotated[str, Form(description="")] = "",
    code_font: Annotated[str, Form(description="")] = "",
    code_font_size: Annotated[str, Form(description="")] = "",
    code_line_height: Annotated[str, Form(description="")] = "",
    code_theme: Annotated[str, Form(description="")] = "",
    margin_top_mm: Annotated[str, Form(description="")] = "",
    margin_right_mm: Annotated[str, Form(description="")] = "",
    margin_bottom_mm: Annotated[str, Form(description="")] = "",
    margin_left_mm: Annotated[str, Form(description="")] = "",
    footer_text: Annotated[str, Form(description="")] = "",
    footer_align: Annotated[str, Form(description="")] = "",
    footer_color: Annotated[str, Form(description="")] = "",
    include_footnotes: Annotated[str, Form(description="")] = "",
    footnote_font_size: Annotated[str, Form(description="")] = "",
    footnote_text_color: Annotated[str, Form(description="")] = "",
    footnote_marker_color: Annotated[str, Form(description="")] = "",
    table_row_stripes: Annotated[str, Form(description="")] = "",
    table_row_odd_color: Annotated[str, Form(description="")] = "",
    table_row_even_color: Annotated[str, Form(description="")] = "",
    table_cell_padding_y_px: Annotated[str, Form(description="")] = "",
    table_cell_padding_x_px: Annotated[str, Form(description="")] = "",
    image_scale: Annotated[str, Form(description="")] = "",
    sanitize_html: Annotated[str, Form(description="")] = "true",
    show_horizontal_rules: Annotated[str, Form(description="")] = "",
    compress_pdf: Annotated[str, Form(description="")] = "",
    compression_profile: Annotated[str, Form(description="balanced | max")] = "",
    strip_metadata: Annotated[str, Form(description="")] = "",
    compression_timeout: Annotated[str, Form(description="")] = "",
    keep_original_on_fail: Annotated[str, Form(description="")] = "",
) -> Response:
    _validate_markdown_text(markdown_text)
    image_mode_value = _image_mode_from_value(image_mode)
    output_format_upper = output_format.upper()
    logger = logging.getLogger(__name__)

    style_input_values: dict[str, object] = {
        "body_font": body_font,
        "body_font_size": body_font_size,
        "line_height": line_height,
        "heading_font": heading_font,
        "heading_color": heading_color,
        "heading_h1_color": heading_h1_color,
        "heading_h2_color": heading_h2_color,
        "heading_h3_color": heading_h3_color,
        "heading_h4_color": heading_h4_color,
        "heading_h5_color": heading_h5_color,
        "heading_h6_color": heading_h6_color,
        "heading_h1_size_px": heading_h1_size_px,
        "heading_h2_size_px": heading_h2_size_px,
        "heading_h3_size_px": heading_h3_size_px,
        "heading_h4_size_px": heading_h4_size_px,
        "heading_h5_size_px": heading_h5_size_px,
        "heading_h6_size_px": heading_h6_size_px,
        "code_font": code_font,
        "code_font_size": code_font_size,
        "code_line_height": code_line_height,
        "code_theme": code_theme,
        "margin_top_mm": margin_top_mm,
        "margin_right_mm": margin_right_mm,
        "margin_bottom_mm": margin_bottom_mm,
        "margin_left_mm": margin_left_mm,
        "footer_text": footer_text,
        "footer_align": footer_align,
        "footer_color": footer_color,
        "include_footnotes": include_footnotes,
        "footnote_font_size": footnote_font_size,
        "footnote_text_color": footnote_text_color,
        "footnote_marker_color": footnote_marker_color,
        "table_row_stripes": table_row_stripes,
        "table_row_odd_color": table_row_odd_color,
        "table_row_even_color": table_row_even_color,
        "table_cell_padding_y_px": table_cell_padding_y_px,
        "table_cell_padding_x_px": table_cell_padding_x_px,
        "image_scale": image_scale,
        "sanitize_html": sanitize_html,
        "show_horizontal_rules": show_horizontal_rules,
        "output_format": output_format,
        "page_size": page_size,
        "compress_pdf": compress_pdf,
        "compression_profile": compression_profile,
        "strip_metadata": strip_metadata,
        "compression_timeout": compression_timeout,
        "keep_original_on_fail": keep_original_on_fail,
    }

    style = _style_from_form_values(style_input_values, preset_name=preset)
    export = _export_from_form_values(style_input_values)

    if image_mode_value == ImageMode.WITH_IMAGES:
        missing = _validate_uploaded_assets_against_markdown(markdown_text, assets)
        if missing:
            return JSONResponse(status_code=422, content={"missing_images": missing})

    tmp_dir = tempfile.mkdtemp(prefix="nectar_render_api_")

    try:
        tmp_path = Path(tmp_dir)
        md_file = tmp_path / "input.md"
        md_file.write_text(markdown_text, encoding="utf-8")

        assets_root: Path | None = None
        if image_mode_value == ImageMode.WITH_IMAGES:
            await _load_assets(
                assets,
                tmp_path=tmp_path,
            )
            assets_root = tmp_path

        service = ConversionService()
        convert_call = partial(
            service.convert,
            markdown_file=md_file,
            output_directory=tmp_path,
            style=style,
            export=export,
            markdown_text=markdown_text,
            api_mode=True,
            image_mode=image_mode_value,
            assets_root=assets_root,
        )
        loop = asyncio.get_running_loop()
        try:
            result = await asyncio.wait_for(
                loop.run_in_executor(None, convert_call),
                timeout=_CONVERSION_TIMEOUT_SECONDS,
            )
        except asyncio.TimeoutError as exc:
            raise HTTPException(status_code=504, detail="Conversion timed out") from exc

        if result.missing_images:
            return JSONResponse(
                status_code=422,
                content={"missing_images": result.missing_images},
                background=_cleanup_task(tmp_dir),
            )

        if output_format_upper == "HTML" and result.html_path:
            return FileResponse(
                path=result.html_path,
                media_type="text/html",
                filename="output.html",
                background=_cleanup_task(tmp_dir),
            )

        if output_format_upper == "PDF+HTML":
            if not result.pdf_path or not result.html_path:
                raise HTTPException(
                    status_code=500,
                    detail={"detail": "Missing output files for PDF+HTML"},
                )

            with tempfile.NamedTemporaryFile(
                mode="w+b",
                suffix=".zip",
                prefix="nectar_render_output_",
                dir=tmp_dir,
                delete=False,
            ) as zip_tmp:
                zip_path = Path(zip_tmp.name)

            with zipfile.ZipFile(zip_path, "w") as archive:
                archive.write(
                    result.pdf_path,
                    arcname="output.pdf",
                    compress_type=zipfile.ZIP_STORED,
                )
                archive.write(
                    result.html_path,
                    arcname="output.html",
                    compress_type=zipfile.ZIP_DEFLATED,
                )

            return FileResponse(
                path=zip_path,
                media_type="application/zip",
                filename="output.zip",
                background=_cleanup_task(tmp_dir),
            )

        if result.pdf_path:
            return FileResponse(
                path=result.pdf_path,
                media_type="application/pdf",
                filename="output.pdf",
                background=_cleanup_task(tmp_dir),
            )

        if result.output_bytes:
            media_type = (
                "text/html" if output_format_upper == "HTML" else "application/pdf"
            )
            filename = "output.html" if output_format_upper == "HTML" else "output.pdf"
            return Response(
                content=result.output_bytes,
                media_type=media_type,
                headers={"Content-Disposition": f'attachment; filename="{filename}"'},
                background=_cleanup_task(tmp_dir),
            )

        raise HTTPException(status_code=500, detail={"detail": "No output generated"})
    except HTTPException:
        shutil.rmtree(tmp_dir, ignore_errors=True)
        raise
    except Exception as exc:
        logger.exception("Conversion failed: %s", exc)
        shutil.rmtree(tmp_dir, ignore_errors=True)
        raise HTTPException(status_code=500, detail={"detail": "Conversion failed"})


@convert_router.post(
    "/preview",
    responses={
        422: {"description": "Missing images"},
        504: {"description": "Preview timeout"},
        500: {"description": "Preview error"},
    },
)
async def preview(request: Request) -> Response:
    form = await request.form()
    markdown_text = str(form.get("markdown_text", ""))
    _validate_markdown_text(markdown_text)

    image_mode_raw = str(form.get("image_mode", "WITH_IMAGES"))
    image_mode_value = _image_mode_from_value(image_mode_raw)

    preview_engine = str(form.get("preview_engine", "html")).strip().lower()
    if preview_engine not in {"html", "pdf"}:
        raise HTTPException(
            status_code=400,
            detail="Invalid preview_engine. Must be html or pdf.",
        )

    preset = str(form.get("preset", ""))
    values: dict[str, object] = {key: form.get(key, "") for key in form.keys()}
    values.setdefault("output_format", "HTML" if preview_engine == "html" else "PDF")

    style = _style_from_form_values(values, preset_name=preset)
    export = _export_from_form_values(values)
    export.output_format = "HTML" if preview_engine == "html" else "PDF"

    uploads = _extract_uploads(form.getlist("assets"))

    if image_mode_value == ImageMode.WITH_IMAGES:
        missing = _validate_uploaded_assets_against_markdown(markdown_text, uploads)
        if missing:
            return JSONResponse(status_code=422, content={"missing_images": missing})

    tmp_dir = tempfile.mkdtemp(prefix="nectar_render_preview_api_")
    logger = logging.getLogger(__name__)

    try:
        tmp_path = Path(tmp_dir)
        md_file = tmp_path / "preview.md"
        md_file.write_text(markdown_text, encoding="utf-8")

        assets_root: Path | None = None
        if image_mode_value == ImageMode.WITH_IMAGES:
            await _load_assets(uploads, tmp_path=tmp_path)
            assets_root = tmp_path

        service = ConversionService()
        convert_call = partial(
            service.convert,
            markdown_file=md_file,
            output_directory=tmp_path,
            style=style,
            export=export,
            markdown_text=markdown_text,
            api_mode=True,
            image_mode=image_mode_value,
            assets_root=assets_root,
            output_bytes=True,
        )
        loop = asyncio.get_running_loop()
        try:
            result = await asyncio.wait_for(
                loop.run_in_executor(None, convert_call),
                timeout=_CONVERSION_TIMEOUT_SECONDS,
            )
        except asyncio.TimeoutError as exc:
            raise HTTPException(status_code=504, detail="Preview timed out") from exc

        if result.missing_images:
            return JSONResponse(
                status_code=422,
                content={"missing_images": result.missing_images},
                background=_cleanup_task(tmp_dir),
            )

        if preview_engine == "html":
            html_content = result.html_preview
            if not html_content and result.output_bytes:
                html_content = result.output_bytes.decode("utf-8", errors="replace")
            if not html_content:
                raise HTTPException(
                    status_code=500, detail={"detail": "No preview generated"}
                )
            return JSONResponse(
                content={
                    "engine": "html",
                    "page_size": export.page_size,
                    "html": html_content,
                },
                background=_cleanup_task(tmp_dir),
            )

        if not result.output_bytes:
            raise HTTPException(
                status_code=500, detail={"detail": "No PDF preview generated"}
            )

        return Response(
            content=result.output_bytes,
            media_type="application/pdf",
            headers={"Content-Disposition": 'inline; filename="preview.pdf"'},
            background=_cleanup_task(tmp_dir),
        )
    except HTTPException:
        shutil.rmtree(tmp_dir, ignore_errors=True)
        raise
    except Exception as exc:
        logger.exception("Preview failed: %s", exc)
        shutil.rmtree(tmp_dir, ignore_errors=True)
        raise HTTPException(status_code=500, detail={"detail": "Preview failed"})
