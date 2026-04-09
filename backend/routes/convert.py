from __future__ import annotations

import asyncio
import logging
import re
import shutil
import tempfile
import time
import zipfile
from collections import deque
from collections.abc import Mapping
from concurrent.futures import ThreadPoolExecutor
from contextlib import suppress
from functools import partial
from pathlib import Path
from typing import Annotated, Any

from fastapi import APIRouter, File, Form, HTTPException, Request, UploadFile
from fastapi.responses import FileResponse, JSONResponse, Response
from starlette.background import BackgroundTask

from nectar_render.application.conversion import (
    ConversionService,
    ImageMode,
    analyze_markdown,
)
from nectar_render.adapters.rendering.image_references import (
    extract_image_references,
    normalize_reference_filename,
)
from nectar_render.core.image_assets import (
    IMAGE_EXTENSIONS,
    is_windows_reserved_filename,
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
_MAX_DATA_URI_CHARS = 350_000
_CONVERSION_TIMEOUT_SECONDS = 120.0
_ASSET_CHUNK_SIZE = 512 * 1024
_MAX_CONCURRENT_CONVERSIONS = 4
_RATE_LIMIT_WINDOW_SECONDS = 60.0
_RATE_LIMIT_MAX_REQUESTS_PER_WINDOW = 30
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
_STYLE_FORM_FIELD_NAMES = (
    "body_font",
    "body_font_size",
    "body_text_color",
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
    "footer_font_size",
    "include_footnotes",
    "footnote_font_size",
    "footnote_text_color",
    "footnote_marker_color",
    "table_row_stripes",
    "table_row_odd_color",
    "table_row_even_color",
    "table_cell_padding_y_px",
    "table_cell_padding_x_px",
    "border_color",
    "image_scale",
    "sanitize_html",
    "show_horizontal_rules",
)

_EXECUTOR = ThreadPoolExecutor(max_workers=_MAX_CONCURRENT_CONVERSIONS)
_CONVERSION_SEMAPHORE = asyncio.Semaphore(_MAX_CONCURRENT_CONVERSIONS)
_RATE_LIMIT_LOCK = asyncio.Lock()
_RATE_LIMIT_BUCKETS: dict[str, deque[float]] = {}
_DATA_URI_RE = re.compile(
    r"data:image/[a-zA-Z0-9.+-]+;base64,([A-Za-z0-9+/=\s]+)",
    re.IGNORECASE,
)


def _cleanup_task(tmp_dir: str) -> BackgroundTask:
    return BackgroundTask(_cleanup_tmp_dir, tmp_dir, logging.getLogger(__name__))


def _parse_bool(value: object, default: bool) -> bool:
    cleaned = str(value or "").strip().lower()
    if cleaned == "":
        return default
    return cleaned not in {"0", "false", "no", "off"}


def _safe_asset_filename(raw_name: str) -> str:
    sanitized = (raw_name or "").replace("\x00", "").strip()
    candidate = Path(sanitized).name
    if not candidate or candidate in {".", ".."}:
        raise HTTPException(status_code=400, detail="Invalid asset filename")
    if is_windows_reserved_filename(candidate):
        raise HTTPException(
            status_code=400,
            detail=f"Asset filename '{candidate}' is reserved on Windows",
        )
    return candidate


def _style_from_form_values(
    values: Mapping[str, Any],
    *,
    preset_name: str,
) -> StyleOptions:
    base_style: StyleOptions | None = None
    if preset_name:
        base_style = get_builtin_preset(preset_name)

    allow_explicit_empty = {
        "footer_text",
        "heading_h1_color",
        "heading_h2_color",
        "heading_h3_color",
        "heading_h4_color",
        "heading_h5_color",
        "heading_h6_color",
    }

    style_mapping = {
        key: value
        for key, value in values.items()
        if key in _STYLE_FORM_FIELD_NAMES
        and (value != "" or key in allow_explicit_empty)
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
    extraction = extract_image_references(markdown_text)
    return extraction.all_references


def _validate_uploaded_assets_against_markdown(
    markdown_text: str,
    uploads: list[UploadFile] | None,
) -> list[str]:
    extraction = extract_image_references(markdown_text)

    uploaded_names: set[str] = set()
    for asset in uploads or []:
        if asset.filename:
            uploaded_names.add(normalize_reference_filename(asset.filename))

    missing: list[str] = []
    seen_missing: set[str] = set()
    for ref in extraction.local_references:
        normalized_ref = normalize_reference_filename(ref)
        if normalized_ref in uploaded_names or normalized_ref in seen_missing:
            continue
        seen_missing.add(normalized_ref)
        missing.append(ref)

    rejected_refs = extraction.rejected_external_references
    if rejected_refs:
        raise HTTPException(
            status_code=400,
            detail=f"External URLs and absolute paths are not allowed: {rejected_refs[:5]}",
        )

    return missing


def _cleanup_tmp_dir(tmp_dir: str, logger: logging.Logger | None = None) -> None:
    active_logger = logger or logging.getLogger(__name__)
    attempts = 4
    for attempt in range(1, attempts + 1):
        try:
            shutil.rmtree(tmp_dir)
            return
        except FileNotFoundError:
            return
        except OSError as exc:
            if attempt == attempts:
                active_logger.warning(
                    "Failed to cleanup temp directory %s after %s attempts: %s",
                    tmp_dir,
                    attempts,
                    exc,
                )
                return
            delay = 0.15 * attempt
            time.sleep(delay)


def _safe_error_message(prefix: str, exc: Exception) -> str:
    raw = str(exc).strip()
    if not raw:
        return prefix
    compact = " ".join(raw.split())
    return f"{prefix}: {compact[:220]}"


async def _enforce_rate_limit(request: Request) -> None:
    client_key = request.client.host if request.client else "unknown"
    now = asyncio.get_running_loop().time()
    async with _RATE_LIMIT_LOCK:
        bucket = _RATE_LIMIT_BUCKETS.setdefault(client_key, deque())
        while bucket and (now - bucket[0]) > _RATE_LIMIT_WINDOW_SECONDS:
            bucket.popleft()
        if len(bucket) >= _RATE_LIMIT_MAX_REQUESTS_PER_WINDOW:
            raise HTTPException(
                status_code=429,
                detail=(
                    "Rate limit exceeded for conversion endpoints. "
                    "Please retry in a minute."
                ),
            )
        bucket.append(now)


def _schedule_cleanup_when_done(
    future: asyncio.Future[Any],
    tmp_dir: str,
    logger: logging.Logger,
) -> None:
    def _cleanup_callback(_done: object) -> None:
        _cleanup_tmp_dir(tmp_dir, logger)

    future.add_done_callback(_cleanup_callback)


async def _close_uploads(uploads: list[UploadFile] | None) -> None:
    for upload in uploads or []:
        with suppress(Exception):
            await upload.close()


async def _run_conversion_call(
    *,
    convert_call: partial[Any],
    tmp_dir: str,
    logger: logging.Logger,
    timeout_message: str,
) -> Any:
    loop = asyncio.get_running_loop()
    async with _CONVERSION_SEMAPHORE:
        future: asyncio.Future[Any] = loop.run_in_executor(_EXECUTOR, convert_call)
        try:
            return await asyncio.wait_for(
                asyncio.shield(future),
                timeout=_CONVERSION_TIMEOUT_SECONDS,
            )
        except asyncio.TimeoutError as exc:
            _schedule_cleanup_when_done(future, tmp_dir, logger)
            raise HTTPException(status_code=504, detail=timeout_message) from exc


def _validate_markdown_text(markdown_text: str) -> None:
    if not markdown_text.strip():
        raise HTTPException(status_code=400, detail="markdown_text cannot be empty")
    if len(markdown_text.encode("utf-8")) > _MAX_MARKDOWN_BYTES:
        raise HTTPException(
            status_code=413,
            detail=f"markdown_text is too large (max {_MAX_MARKDOWN_BYTES} bytes)",
        )
    for match in _DATA_URI_RE.finditer(markdown_text):
        payload = "".join((match.group(1) or "").split())
        if len(payload) > _MAX_DATA_URI_CHARS:
            raise HTTPException(
                status_code=413,
                detail=(
                    "Embedded data URI is too large. "
                    f"Maximum base64 payload is {_MAX_DATA_URI_CHARS} characters."
                ),
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
        if extension not in IMAGE_EXTENSIONS:
            raise HTTPException(
                status_code=400,
                detail=f"Unsupported asset type for '{filename}'",
            )

        if filename in asset_names:
            raise HTTPException(
                status_code=400,
                detail=f"Duplicate asset filename '{filename}'",
            )

        target = (tmp_path / filename).resolve()
        if target.parent != tmp_root:
            raise HTTPException(status_code=400, detail="Invalid asset path")
        file_size = 0
        with target.open("wb") as handle:
            while True:
                chunk = await asset.read(_ASSET_CHUNK_SIZE)
                if not chunk:
                    break
                file_size += len(chunk)
                if file_size > _MAX_ASSET_BYTES:
                    handle.close()
                    target.unlink(missing_ok=True)
                    raise HTTPException(
                        status_code=413,
                        detail=f"Asset '{filename}' exceeds {_MAX_ASSET_BYTES} bytes",
                    )
                total_bytes += len(chunk)
                if total_bytes > _MAX_ASSETS_TOTAL_BYTES:
                    handle.close()
                    target.unlink(missing_ok=True)
                    raise HTTPException(
                        status_code=413,
                        detail=(
                            f"Total asset payload exceeds {_MAX_ASSETS_TOTAL_BYTES} bytes"
                        ),
                    )
                handle.write(chunk)

        asset_names.add(filename)

    return asset_names


def _extract_uploads(values: list[object]) -> list[UploadFile]:
    uploads: list[UploadFile] = []
    for value in values:
        if hasattr(value, "filename") and hasattr(value, "read"):
            uploads.append(value)  # type: ignore[arg-type]
    return uploads


def _response_with_cleanup(response: Response, tmp_dir: str) -> Response:
    response.background = _cleanup_task(tmp_dir)
    return response


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
    extraction = extract_image_references(markdown_text)
    if extraction.rejected_external_references:
        raise HTTPException(
            status_code=400,
            detail=(
                "External URLs and absolute paths are not allowed: "
                f"{extraction.rejected_external_references[:5]}"
            ),
        )
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
    request: Request,
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
    body_text_color: Annotated[str, Form(description="")] = "",
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
    footer_font_size: Annotated[str, Form(description="")] = "",
    include_footnotes: Annotated[str, Form(description="")] = "",
    footnote_font_size: Annotated[str, Form(description="")] = "",
    footnote_text_color: Annotated[str, Form(description="")] = "",
    footnote_marker_color: Annotated[str, Form(description="")] = "",
    table_row_stripes: Annotated[str, Form(description="")] = "",
    table_row_odd_color: Annotated[str, Form(description="")] = "",
    table_row_even_color: Annotated[str, Form(description="")] = "",
    table_cell_padding_y_px: Annotated[str, Form(description="")] = "",
    table_cell_padding_x_px: Annotated[str, Form(description="")] = "",
    border_color: Annotated[str, Form(description="")] = "",
    image_scale: Annotated[str, Form(description="")] = "",
    sanitize_html: Annotated[str, Form(description="")] = "",
    show_horizontal_rules: Annotated[str, Form(description="")] = "",
    compress_pdf: Annotated[str, Form(description="")] = "",
    compression_profile: Annotated[str, Form(description="balanced | max")] = "",
    strip_metadata: Annotated[str, Form(description="")] = "",
    compression_timeout: Annotated[str, Form(description="")] = "",
    keep_original_on_fail: Annotated[str, Form(description="")] = "",
) -> Response:
    await _enforce_rate_limit(request)
    _validate_markdown_text(markdown_text)
    image_mode_value = _image_mode_from_value(image_mode)
    output_format_upper = output_format.upper()
    logger = logging.getLogger(__name__)

    style_input_values: dict[str, object] = {
        "body_font": body_font,
        "body_font_size": body_font_size,
        "body_text_color": body_text_color,
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
        "footer_font_size": footer_font_size,
        "include_footnotes": include_footnotes,
        "footnote_font_size": footnote_font_size,
        "footnote_text_color": footnote_text_color,
        "footnote_marker_color": footnote_marker_color,
        "table_row_stripes": table_row_stripes,
        "table_row_odd_color": table_row_odd_color,
        "table_row_even_color": table_row_even_color,
        "table_cell_padding_y_px": table_cell_padding_y_px,
        "table_cell_padding_x_px": table_cell_padding_x_px,
        "border_color": border_color,
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
        await asyncio.to_thread(md_file.write_text, markdown_text, encoding="utf-8")

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
        result = await _run_conversion_call(
            convert_call=convert_call,
            tmp_dir=tmp_dir,
            logger=logger,
            timeout_message="Conversion timed out",
        )

        if result.missing_images:
            return _response_with_cleanup(
                JSONResponse(
                    status_code=422,
                    content={"missing_images": result.missing_images},
                ),
                tmp_dir,
            )

        if output_format_upper == "HTML" and result.html_path:
            return _response_with_cleanup(
                FileResponse(
                    path=result.html_path,
                    media_type="text/html",
                    filename="output.html",
                ),
                tmp_dir,
            )

        if output_format_upper == "PDF+HTML":
            if not result.pdf_path or not result.html_path:
                raise HTTPException(
                    status_code=500,
                    detail="Missing output files for PDF+HTML",
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

            return _response_with_cleanup(
                FileResponse(
                    path=zip_path,
                    media_type="application/zip",
                    filename="output.zip",
                ),
                tmp_dir,
            )

        if result.pdf_path:
            return _response_with_cleanup(
                FileResponse(
                    path=result.pdf_path,
                    media_type="application/pdf",
                    filename="output.pdf",
                ),
                tmp_dir,
            )

        if result.output_bytes:
            media_type = (
                "text/html" if output_format_upper == "HTML" else "application/pdf"
            )
            filename = "output.html" if output_format_upper == "HTML" else "output.pdf"
            return _response_with_cleanup(
                Response(
                    content=result.output_bytes,
                    media_type=media_type,
                    headers={
                        "Content-Disposition": f'attachment; filename="{filename}"'
                    },
                ),
                tmp_dir,
            )

        raise HTTPException(status_code=500, detail="No output generated")
    except HTTPException:
        _cleanup_tmp_dir(tmp_dir, logger)
        await _close_uploads(assets)
        raise
    except Exception as exc:
        logger.exception("Conversion failed: %s", exc)
        _cleanup_tmp_dir(tmp_dir, logger)
        await _close_uploads(assets)
        raise HTTPException(
            status_code=500,
            detail=_safe_error_message("Conversion failed", exc),
        )
    finally:
        await _close_uploads(assets)


@convert_router.post(
    "/preview",
    responses={
        422: {"description": "Missing images"},
        504: {"description": "Preview timeout"},
        500: {"description": "Preview error"},
    },
)
async def preview(request: Request) -> Response:
    await _enforce_rate_limit(request)
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
    if preview_engine == "html":
        style.sanitize_html = True
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
        await asyncio.to_thread(md_file.write_text, markdown_text, encoding="utf-8")

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
        result = await _run_conversion_call(
            convert_call=convert_call,
            tmp_dir=tmp_dir,
            logger=logger,
            timeout_message="Preview timed out",
        )

        if result.missing_images:
            return _response_with_cleanup(
                JSONResponse(
                    status_code=422,
                    content={"missing_images": result.missing_images},
                ),
                tmp_dir,
            )

        if preview_engine == "html":
            html_content = result.html_preview
            if not html_content and result.output_bytes:
                html_content = result.output_bytes.decode("utf-8", errors="replace")
            if not html_content:
                raise HTTPException(status_code=500, detail="No preview generated")
            return _response_with_cleanup(
                JSONResponse(
                    content={
                        "engine": "html",
                        "page_size": export.page_size,
                        "html": html_content,
                    },
                ),
                tmp_dir,
            )

        if not result.output_bytes:
            raise HTTPException(status_code=500, detail="No PDF preview generated")

        return _response_with_cleanup(
            Response(
                content=result.output_bytes,
                media_type="application/pdf",
                headers={"Content-Disposition": 'inline; filename="preview.pdf"'},
            ),
            tmp_dir,
        )
    except HTTPException:
        _cleanup_tmp_dir(tmp_dir, logger)
        await _close_uploads(uploads)
        raise
    except Exception as exc:
        logger.exception("Preview failed: %s", exc)
        _cleanup_tmp_dir(tmp_dir, logger)
        await _close_uploads(uploads)
        raise HTTPException(
            status_code=500,
            detail=_safe_error_message("Preview failed", exc),
        )
    finally:
        await _close_uploads(uploads)
