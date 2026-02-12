from __future__ import annotations

import logging
import shutil
import subprocess
import tempfile
import importlib
from dataclasses import dataclass
from pathlib import Path

from ..config import CompressionOptions


logger = logging.getLogger(__name__)


@dataclass(slots=True)
class PdfCompressionResult:
    path: Path
    applied: bool
    original_size: int
    final_size: int
    tool: str | None = None


class PdfCompressionService:
    def compress(self, pdf_path: Path, options: CompressionOptions) -> PdfCompressionResult:
        original_size = pdf_path.stat().st_size

        if not options.enabled:
            return PdfCompressionResult(
                path=pdf_path,
                applied=False,
                original_size=original_size,
                final_size=original_size,
            )

        working_path = pdf_path
        tool_used: str | None = None

        qpdf_path = shutil.which("qpdf")
        if qpdf_path:
            compressed = self._run_qpdf(
                qpdf_path=qpdf_path,
                source_path=working_path,
                profile=options.profile,
                timeout_sec=max(5, options.timeout_sec),
            )
            if compressed is not None:
                working_path = compressed
                tool_used = "qpdf"
            else:
                logger.warning("qpdf compression unavailable/failed, keeping original PDF: %s", pdf_path)

        if options.remove_metadata:
            metadata_clean = self._remove_metadata(working_path)
            if metadata_clean is not None:
                working_path = metadata_clean
                tool_used = tool_used or "pypdf"

        if working_path != pdf_path:
            final_size = working_path.stat().st_size
            if final_size <= original_size:
                pdf_path.unlink(missing_ok=True)
                working_path.replace(pdf_path)
                applied = final_size < original_size
                return PdfCompressionResult(
                    path=pdf_path,
                    applied=applied,
                    original_size=original_size,
                    final_size=final_size,
                    tool=tool_used,
                )

            working_path.unlink(missing_ok=True)

        return PdfCompressionResult(
            path=pdf_path,
            applied=False,
            original_size=original_size,
            final_size=original_size,
            tool=tool_used,
        )

    def _run_qpdf(self, qpdf_path: str, source_path: Path, profile: str, timeout_sec: int) -> Path | None:
        compression_level = "9" if profile.lower() == "max" else "6"
        with tempfile.NamedTemporaryFile(
            mode="wb",
            suffix=".pdf",
            prefix="md_to_pdf_comp_",
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
        except Exception:
            temp_path.unlink(missing_ok=True)
            logger.exception("Error running qpdf")
            return None

        if completed.returncode != 0:
            temp_path.unlink(missing_ok=True)
            logger.warning("qpdf returned non-zero exit code (%s): %s", completed.returncode, completed.stderr.strip())
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
            prefix="md_to_pdf_meta_",
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
