from __future__ import annotations

import asyncio
import io
import logging
import re
import shutil
import tempfile
import zipfile
from functools import partial
from pathlib import Path
from typing import Annotated

from fastapi import APIRouter, File, Form, HTTPException, UploadFile
from fastapi.responses import JSONResponse, Response, StreamingResponse
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

router = APIRouter(prefix="/analyze", tags=["conversion"])
convert_router = APIRouter(tags=["conversion"])

_MAX_MARKDOWN_BYTES = 2 * 1024 * 1024
_MAX_ASSET_COUNT = 128
_MAX_ASSET_BYTES = 10 * 1024 * 1024
_MAX_ASSETS_TOTAL_BYTES = 50 * 1024 * 1024
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
    ".tif",
    ".tiff",
    ".webp",
}


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
) -> dict[str, bytes]:
    if not uploads:
        return {}

    if any(
        asset.filename and Path(asset.filename).suffix.lower() == ".svg"
        for asset in uploads
    ):
        raise HTTPException(
            status_code=400,
            detail="SVG files are not allowed as assets.",
        )

    if len(uploads) > _MAX_ASSET_COUNT:
        raise HTTPException(
            status_code=413,
            detail=f"Too many assets (max {_MAX_ASSET_COUNT})",
        )

    assets: dict[str, bytes] = {}
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

        if filename in assets:
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
        assets[filename] = data

    return assets


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
    sanitize_html: Annotated[str, Form(description="")] = "",
    show_horizontal_rules: Annotated[str, Form(description="")] = "",
    compress_pdf: Annotated[str, Form(description="")] = "",
    strip_metadata: Annotated[str, Form(description="")] = "",
) -> Response:
    _validate_markdown_text(markdown_text)

    image_mode_upper = image_mode.upper()
    image_mode_value = _IMAGE_MODE_MAP.get(image_mode_upper)
    if image_mode_value is None:
        raise HTTPException(
            status_code=400,
            detail="Invalid image_mode. Must be WITH_IMAGES, ALT_ONLY, or STRIP.",
        )

    output_format_upper = output_format.upper()
    if output_format_upper not in _OUTPUT_FORMATS:
        raise HTTPException(
            status_code=400,
            detail="Invalid output_format. Must be PDF, HTML, or PDF+HTML.",
        )

    page_size_upper = page_size.upper()
    page_size_value = _PAGE_SIZE_MAP.get(page_size_upper)
    if page_size_value is None:
        raise HTTPException(
            status_code=400,
            detail="Invalid page_size. Must be A4, Letter, Legal, A3, or A5.",
        )

    base_style: StyleOptions | None = None
    if preset:
        base_style = get_builtin_preset(preset)
        if base_style is None:
            raise HTTPException(status_code=400, detail=f"Unknown preset '{preset}'")

    style_mapping: dict[str, object] = {
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
    }
    style_mapping_clean = {k: v for k, v in style_mapping.items() if v != ""}
    style = style_from_option_mapping(style_mapping_clean, base_style=base_style)

    export = ExportOptions(
        output_format=output_format_upper,
        page_size=page_size_value,
        compression=CompressionOptions(
            enabled=_parse_bool(compress_pdf, True),
            remove_metadata=_parse_bool(strip_metadata, True),
        ),
    )

    if assets and any(
        asset.filename and Path(asset.filename).suffix.lower() == ".svg"
        for asset in assets
    ):
        raise HTTPException(
            status_code=400,
            detail="SVG files are not allowed as assets.",
        )

    if image_mode_value == ImageMode.WITH_IMAGES:
        standard = re.findall(r"!\[.*?\]\(([^)]+)\)", markdown_text)
        obsidian = re.findall(r"!\[\[([^\]|]+)(?:\|[^\]]*)?\]\]", markdown_text)
        all_refs = [ref.strip() for ref in standard + obsidian if ref.strip()]
        uploaded_names = {
            Path(asset.filename).name for asset in (assets or []) if asset.filename
        }
        missing: list[str] = []
        seen_missing: set[str] = set()
        for ref in all_refs:
            if ref.startswith("http://") or ref.startswith("https://"):
                continue
            if Path(ref).name in uploaded_names or ref in seen_missing:
                continue
            seen_missing.add(ref)
            missing.append(ref)
        if missing:
            return JSONResponse(status_code=422, content={"missing_images": missing})

    tmp_dir = tempfile.mkdtemp(prefix="nectar_render_api_")
    logger = logging.getLogger(__name__)

    try:
        tmp_path = Path(tmp_dir)
        md_file = tmp_path / "input.md"
        md_file.write_text(markdown_text, encoding="utf-8")

        assets_dict: dict[str, bytes] = {}
        if image_mode_value == ImageMode.WITH_IMAGES:
            assets_dict = await _load_assets(
                assets,
                tmp_path=tmp_path,
            )

        service = ConversionService()
        convert_call = partial(
            service.convert,
            markdown_file=md_file,
            output_directory=tmp_path,
            style=style,
            export=export,
            markdown_text=markdown_text,
            image_mode=image_mode_value,
            assets=assets_dict if image_mode_value == ImageMode.WITH_IMAGES else None,
        )
        loop = asyncio.get_running_loop()
        result = await loop.run_in_executor(None, convert_call)

        if result.missing_images:
            return JSONResponse(
                status_code=422,
                content={"missing_images": result.missing_images},
            )

        if output_format_upper == "HTML" and result.html_path:
            html_bytes = result.html_path.read_bytes()
            return StreamingResponse(
                iter([html_bytes]),
                media_type="text/html",
                headers={"Content-Disposition": 'attachment; filename="output.html"'},
                background=_cleanup_task(tmp_dir),
            )

        if output_format_upper == "PDF+HTML":
            if not result.pdf_path or not result.html_path:
                raise HTTPException(
                    status_code=500,
                    detail={"detail": "Missing output files for PDF+HTML"},
                )

            zip_buffer = io.BytesIO()
            with zipfile.ZipFile(zip_buffer, "w", zipfile.ZIP_DEFLATED) as archive:
                archive.writestr("output.pdf", result.pdf_path.read_bytes())
                archive.writestr("output.html", result.html_path.read_bytes())
            zip_buffer.seek(0)
            return StreamingResponse(
                iter([zip_buffer.getvalue()]),
                media_type="application/zip",
                headers={"Content-Disposition": 'attachment; filename="output.zip"'},
                background=_cleanup_task(tmp_dir),
            )

        if result.pdf_path:
            pdf_bytes = result.pdf_path.read_bytes()
            return StreamingResponse(
                iter([pdf_bytes]),
                media_type="application/pdf",
                headers={"Content-Disposition": 'attachment; filename="output.pdf"'},
                background=_cleanup_task(tmp_dir),
            )

        if result.output_bytes:
            media_type = (
                "text/html" if output_format_upper == "HTML" else "application/pdf"
            )
            filename = "output.html" if output_format_upper == "HTML" else "output.pdf"
            return StreamingResponse(
                iter([result.output_bytes]),
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
