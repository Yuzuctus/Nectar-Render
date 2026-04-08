from __future__ import annotations

import importlib
import logging
import shutil
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path

from ..core.styles import CompressionOptions


logger = logging.getLogger(__name__)


@dataclass(slots=True)
class PdfCompressionResult:
    path: Path
    applied: bool
    original_size: int
    final_size: int
    tool: str | None = None


class PdfCompressionService:
    def compress(
        self, pdf_path: Path, options: CompressionOptions
    ) -> PdfCompressionResult:
        original_size = pdf_path.stat().st_size

        if not options.enabled and not options.remove_metadata:
            return PdfCompressionResult(
                path=pdf_path,
                applied=False,
                original_size=original_size,
                final_size=original_size,
            )

        working_path = pdf_path
        tool_used: str | None = None

        if options.enabled:
            qpdf_path = shutil.which("qpdf")
            if qpdf_path:
                compressed = self._run_qpdf(
                    qpdf_path=qpdf_path,
                    source_path=working_path,
                    profile=options.profile,
                    timeout_sec=max(5, options.timeout_sec),
                )
                if compressed is not None:
                    compressed_size = compressed.stat().st_size
                    if compressed_size < original_size:
                        working_path = compressed
                        tool_used = "qpdf"
                    else:
                        compressed.unlink(missing_ok=True)
                        logger.info(
                            "Skipping qpdf output because it is not smaller than the original: %s",
                            pdf_path,
                        )
                else:
                    logger.warning(
                        "qpdf compression unavailable/failed, keeping original PDF: %s",
                        pdf_path,
                    )

        if options.remove_metadata:
            metadata_clean = self._remove_metadata(working_path)
            if metadata_clean is not None:
                metadata_size = metadata_clean.stat().st_size
                if metadata_size <= original_size:
                    if working_path != pdf_path:
                        working_path.unlink(missing_ok=True)
                    working_path = metadata_clean
                    tool_used = tool_used or "pypdf"
                else:
                    metadata_clean.unlink(missing_ok=True)
                    logger.info(
                        "Skipping metadata-cleaned PDF because it is larger than the original: %s",
                        pdf_path,
                    )

        if working_path != pdf_path:
            final_size = working_path.stat().st_size
            try:
                working_path.replace(pdf_path)
            except Exception:
                logger.exception(
                    "Error replacing original PDF with compressed copy: %s",
                    pdf_path,
                )
                working_path.unlink(missing_ok=True)
                if options.keep_original_on_fail and pdf_path.exists():
                    return PdfCompressionResult(
                        path=pdf_path,
                        applied=False,
                        original_size=original_size,
                        final_size=original_size,
                        tool=tool_used,
                    )
                raise

            if not pdf_path.exists():
                msg = (
                    "Compressed PDF promotion finished without a final output file: "
                    f"{pdf_path}"
                )
                logger.error(msg)
                raise FileNotFoundError(msg)

            return PdfCompressionResult(
                path=pdf_path,
                applied=True,
                original_size=original_size,
                final_size=final_size,
                tool=tool_used,
            )

        return PdfCompressionResult(
            path=pdf_path,
            applied=False,
            original_size=original_size,
            final_size=original_size,
            tool=tool_used,
        )

    def _run_qpdf(
        self, qpdf_path: str, source_path: Path, profile: str, timeout_sec: int
    ) -> Path | None:
        compression_level = "9" if profile.lower() == "max" else "6"
        with tempfile.NamedTemporaryFile(
            mode="wb",
            suffix=".pdf",
            prefix="nectar_render_comp_",
            dir=source_path.parent,
            delete=False,
        ) as tmp_file:
            temp_path = Path(tmp_file.name)

        cmd = [
            qpdf_path,
            "--warning-exit-0",
            "--object-streams=generate",
            "--compress-streams=y",
            "--recompress-flate",
            f"--compression-level={compression_level}",
            "--deterministic-id",
            str(source_path),
            str(temp_path),
        ]

        try:
            completed = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=timeout_sec,
                check=False,
            )
        except subprocess.TimeoutExpired as exc:
            temp_path.unlink(missing_ok=True)
            logger.warning("qpdf timed out after %s seconds: %s", timeout_sec, exc)
            return None
        except OSError as exc:
            temp_path.unlink(missing_ok=True)
            logger.warning("Failed to run qpdf: %s", exc)
            return None

        if completed.returncode != 0:
            temp_path.unlink(missing_ok=True)
            logger.warning(
                "qpdf returned non-zero exit code (%s): %s",
                completed.returncode,
                completed.stderr.strip(),
            )
            return None

        if not temp_path.exists() or temp_path.stat().st_size <= 0:
            temp_path.unlink(missing_ok=True)
            return None
        return temp_path

    def _remove_metadata(self, source_path: Path) -> Path | None:
        try:
            pypdf = importlib.import_module("pypdf")
        except Exception:
            logger.info("pypdf not installed, skipping metadata removal")
            return None

        PdfReader = getattr(pypdf, "PdfReader", None)
        PdfWriter = getattr(pypdf, "PdfWriter", None)
        if PdfReader is None or PdfWriter is None:
            logger.info("pypdf installed but PdfReader/PdfWriter API unavailable")
            return None

        with tempfile.NamedTemporaryFile(
            mode="wb",
            suffix=".pdf",
            prefix="nectar_render_meta_",
            dir=source_path.parent,
            delete=False,
        ) as tmp_file:
            temp_path = Path(tmp_file.name)

        try:
            reader = PdfReader(str(source_path))
            writer = PdfWriter()
            for page in reader.pages:
                writer.add_page(page)
            writer.add_metadata({})
            with temp_path.open("wb") as out_file:
                writer.write(out_file)
        except Exception:
            temp_path.unlink(missing_ok=True)
            logger.exception("Error removing PDF metadata")
            return None

        if not temp_path.exists() or temp_path.stat().st_size <= 0:
            temp_path.unlink(missing_ok=True)
            return None
        return temp_path


__all__ = ["PdfCompressionResult", "PdfCompressionService"]
