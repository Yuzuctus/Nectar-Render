from __future__ import annotations

from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest
from pypdf import PdfWriter

from nectar_render.config import CompressionOptions
from nectar_render.services.pdf_compression_service import PdfCompressionService


@pytest.fixture
def service():
    return PdfCompressionService()


@pytest.fixture
def dummy_pdf(tmp_path: Path) -> Path:
    pdf_path = tmp_path / "test.pdf"
    pdf_path.write_bytes(b"%PDF-1.4 fake content " * 100)
    return pdf_path


class TestCompressionDisabled:
    def test_returns_original_when_disabled(self, service, dummy_pdf):
        options = CompressionOptions(enabled=False)
        result = service.compress(dummy_pdf, options)
        assert not result.applied
        assert result.path == dummy_pdf
        assert result.original_size == result.final_size

    def test_original_file_unchanged(self, service, dummy_pdf):
        original_bytes = dummy_pdf.read_bytes()
        options = CompressionOptions(enabled=False)
        service.compress(dummy_pdf, options)
        assert dummy_pdf.read_bytes() == original_bytes

    @patch("shutil.which", return_value=None)
    def test_remove_metadata_still_runs_when_compression_disabled(
        self, mock_which, service, tmp_path: Path, monkeypatch
    ):
        pdf_path = tmp_path / "with-metadata.pdf"
        writer = PdfWriter()
        writer.add_blank_page(width=72, height=72)
        writer.add_metadata({"/Title": "Demo"})
        with pdf_path.open("wb") as handle:
            writer.write(handle)

        original_size = pdf_path.stat().st_size

        def fake_remove_metadata(source_path: Path) -> Path:
            temp_path = source_path.parent / "metadata-clean.pdf"
            temp_path.write_bytes(b"x" * (original_size - 16))
            return temp_path

        monkeypatch.setattr(service, "_remove_metadata", fake_remove_metadata)
        result = service.compress(
            pdf_path,
            CompressionOptions(enabled=False, remove_metadata=True),
        )

        assert result.path == pdf_path
        assert result.applied is True
        assert result.tool == "pypdf"
        assert result.final_size < original_size

    @patch("shutil.which", return_value=None)
    def test_remove_metadata_larger_output_keeps_original(
        self, mock_which, service, tmp_path: Path, monkeypatch
    ):
        pdf_path = tmp_path / "with-metadata.pdf"
        writer = PdfWriter()
        writer.add_blank_page(width=72, height=72)
        writer.add_metadata({"/Title": "Demo"})
        with pdf_path.open("wb") as handle:
            writer.write(handle)

        original_bytes = pdf_path.read_bytes()
        original_size = pdf_path.stat().st_size

        def fake_remove_metadata(source_path: Path) -> Path:
            temp_path = source_path.parent / "metadata-clean-larger.pdf"
            temp_path.write_bytes(b"y" * (original_size + 32))
            return temp_path

        monkeypatch.setattr(service, "_remove_metadata", fake_remove_metadata)
        result = service.compress(
            pdf_path,
            CompressionOptions(enabled=False, remove_metadata=True),
        )

        assert result.path == pdf_path
        assert result.applied is False
        assert result.final_size == original_size
        assert pdf_path.read_bytes() == original_bytes
        assert not (tmp_path / "metadata-clean-larger.pdf").exists()


class TestQpdfCompression:
    @patch("shutil.which", return_value="/usr/bin/qpdf")
    @patch("subprocess.run")
    def test_qpdf_success_smaller_output(
        self, mock_run, mock_which, service, dummy_pdf
    ):
        original_size = dummy_pdf.stat().st_size

        def side_effect(cmd, **kwargs):
            # Write a smaller file to the temp path (last arg)
            temp_path = Path(cmd[-1])
            temp_path.write_bytes(b"%PDF-1.4 small")
            return MagicMock(returncode=0, stderr="")

        mock_run.side_effect = side_effect
        options = CompressionOptions(
            enabled=True, profile="balanced", remove_metadata=False
        )
        result = service.compress(dummy_pdf, options)

        assert result.applied
        assert result.tool == "qpdf"
        assert result.final_size < original_size

    @patch("shutil.which", return_value="/usr/bin/qpdf")
    @patch("subprocess.run")
    def test_qpdf_larger_output_keeps_original(
        self, mock_run, mock_which, service, dummy_pdf
    ):
        original_size = dummy_pdf.stat().st_size

        def side_effect(cmd, **kwargs):
            temp_path = Path(cmd[-1])
            temp_path.write_bytes(b"x" * (original_size * 2))
            return MagicMock(returncode=0, stderr="")

        mock_run.side_effect = side_effect
        options = CompressionOptions(
            enabled=True, profile="balanced", remove_metadata=False
        )
        result = service.compress(dummy_pdf, options)

        assert not result.applied
        assert result.final_size == original_size

    @patch("shutil.which", return_value="/usr/bin/qpdf")
    @patch("subprocess.run")
    def test_metadata_cleanup_does_not_promote_larger_pdf(
        self, mock_run, mock_which, service, dummy_pdf, monkeypatch
    ):
        original_size = dummy_pdf.stat().st_size

        def run_side_effect(cmd, **kwargs):
            temp_path = Path(cmd[-1])
            temp_path.write_bytes(b"x" * (original_size * 2))
            return MagicMock(returncode=0, stderr="")

        def remove_metadata_side_effect(source_path: Path):
            temp_path = source_path.parent / "metadata-clean.pdf"
            temp_path.write_bytes(b"y" * (original_size + 10))
            return temp_path

        mock_run.side_effect = run_side_effect
        monkeypatch.setattr(service, "_remove_metadata", remove_metadata_side_effect)

        result = service.compress(
            dummy_pdf,
            CompressionOptions(enabled=True, remove_metadata=True),
        )

        assert not result.applied
        assert result.path == dummy_pdf
        assert result.final_size == original_size
        assert dummy_pdf.stat().st_size == original_size
        assert not (dummy_pdf.parent / "metadata-clean.pdf").exists()

    @patch("shutil.which", return_value="/usr/bin/qpdf")
    @patch("subprocess.run")
    def test_qpdf_then_metadata_growth_keeps_smaller_qpdf_result(
        self, mock_run, mock_which, service, dummy_pdf, monkeypatch
    ):
        original_size = dummy_pdf.stat().st_size

        def qpdf_side_effect(cmd, **kwargs):
            temp_path = Path(cmd[-1])
            temp_path.write_bytes(b"s" * (original_size - 64))
            return MagicMock(returncode=0, stderr="")

        def fake_remove_metadata(source_path: Path) -> Path:
            temp_path = source_path.parent / "metadata-growth.pdf"
            temp_path.write_bytes(b"m" * (original_size + 64))
            return temp_path

        mock_run.side_effect = qpdf_side_effect
        monkeypatch.setattr(service, "_remove_metadata", fake_remove_metadata)
        result = service.compress(
            dummy_pdf,
            CompressionOptions(enabled=True, profile="balanced", remove_metadata=True),
        )

        assert result.path == dummy_pdf
        assert result.applied is True
        assert result.tool == "qpdf"
        assert result.final_size < original_size
        assert dummy_pdf.stat().st_size == result.final_size
        assert not (dummy_pdf.parent / "metadata-growth.pdf").exists()

    @patch("shutil.which", return_value="/usr/bin/qpdf")
    @patch("subprocess.run")
    def test_qpdf_failure_returns_original(
        self, mock_run, mock_which, service, dummy_pdf
    ):
        mock_run.return_value = MagicMock(returncode=1, stderr="error")
        options = CompressionOptions(enabled=True, profile="max", remove_metadata=False)
        result = service.compress(dummy_pdf, options)

        assert not result.applied
        assert result.path == dummy_pdf

    @patch("shutil.which", return_value="/usr/bin/qpdf")
    @patch("subprocess.run", side_effect=Exception("timeout"))
    def test_qpdf_exception_returns_original(
        self, mock_run, mock_which, service, dummy_pdf
    ):
        options = CompressionOptions(enabled=True, remove_metadata=False)
        result = service.compress(dummy_pdf, options)

        assert not result.applied
        assert result.path == dummy_pdf

    @patch("shutil.which", return_value="/usr/bin/qpdf")
    @patch("subprocess.run")
    def test_max_profile_uses_compression_level_9(
        self, mock_run, mock_which, service, dummy_pdf
    ):
        mock_run.return_value = MagicMock(returncode=1, stderr="")
        options = CompressionOptions(enabled=True, profile="max", remove_metadata=False)
        service.compress(dummy_pdf, options)

        cmd = mock_run.call_args[0][0]
        assert "--compression-level=9" in cmd

    @patch("shutil.which", return_value="/usr/bin/qpdf")
    @patch("subprocess.run")
    def test_balanced_profile_uses_compression_level_6(
        self, mock_run, mock_which, service, dummy_pdf
    ):
        mock_run.return_value = MagicMock(returncode=1, stderr="")
        options = CompressionOptions(
            enabled=True, profile="balanced", remove_metadata=False
        )
        service.compress(dummy_pdf, options)

        cmd = mock_run.call_args[0][0]
        assert "--compression-level=6" in cmd


class TestNoQpdf:
    @patch("shutil.which", return_value=None)
    def test_no_qpdf_skips_compression(self, mock_which, service, dummy_pdf):
        options = CompressionOptions(enabled=True, remove_metadata=False)
        result = service.compress(dummy_pdf, options)

        assert not result.applied
        assert result.path == dummy_pdf


class TestMetadataRemoval:
    @patch("shutil.which", return_value=None)
    def test_metadata_removal_fallback(self, mock_which, service, dummy_pdf):
        """When pypdf is not available, metadata removal is skipped gracefully."""
        options = CompressionOptions(enabled=True, remove_metadata=True)
        result = service.compress(dummy_pdf, options)
        # Should not crash even if pypdf produces an error on fake PDF
        assert result.path == dummy_pdf


class TestTempFileCleanup:
    @patch("shutil.which", return_value="/usr/bin/qpdf")
    @patch("subprocess.run")
    def test_temp_files_cleaned_on_qpdf_failure(
        self, mock_run, mock_which, service, dummy_pdf
    ):
        mock_run.return_value = MagicMock(returncode=1, stderr="error")
        options = CompressionOptions(enabled=True, remove_metadata=False)
        service.compress(dummy_pdf, options)

        # No temp files should remain in the directory
        temp_files = list(dummy_pdf.parent.glob("nectar_render_comp_*"))
        assert len(temp_files) == 0

    @patch("shutil.which", return_value="/usr/bin/qpdf")
    @patch("subprocess.run")
    def test_replace_failure_keeps_original_pdf_when_requested(
        self, mock_run, mock_which, service, dummy_pdf, monkeypatch
    ):
        original_bytes = dummy_pdf.read_bytes()
        original_replace = Path.replace

        def run_side_effect(cmd, **kwargs):
            temp_path = Path(cmd[-1])
            temp_path.write_bytes(b"%PDF-1.4 small")
            return MagicMock(returncode=0, stderr="")

        def replace_side_effect(self, target):
            if self != dummy_pdf and target == dummy_pdf:
                raise PermissionError("locked")
            return original_replace(self, target)

        mock_run.side_effect = run_side_effect
        monkeypatch.setattr(Path, "replace", replace_side_effect)

        options = CompressionOptions(
            enabled=True,
            remove_metadata=False,
            keep_original_on_fail=True,
        )
        result = service.compress(dummy_pdf, options)

        assert not result.applied
        assert result.path == dummy_pdf
        assert dummy_pdf.exists()
        assert dummy_pdf.read_bytes() == original_bytes
        temp_files = list(dummy_pdf.parent.glob("nectar_render_comp_*"))
        assert len(temp_files) == 0
