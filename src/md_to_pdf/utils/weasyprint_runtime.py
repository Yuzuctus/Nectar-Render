from __future__ import annotations

import os
import sys
from dataclasses import dataclass
from pathlib import Path


_DLL_DIRECTORY_HANDLES: list[object] = []
_REGISTERED_DLL_DIRECTORIES: set[str] = set()


@dataclass(frozen=True, slots=True)
class WeasyPrintRuntimeStatus:
    configured_directories: tuple[Path, ...]
    searched_directories: tuple[Path, ...]


class WeasyPrintRuntimeError(RuntimeError):
    """Raised when Windows native dependencies required by WeasyPrint are missing."""


def _path_key(path: Path) -> str:
    return os.path.normcase(os.path.abspath(os.fspath(path)))


def _dedupe_paths(paths: list[Path]) -> list[Path]:
    unique: list[Path] = []
    seen: set[str] = set()
    for path in paths:
        key = _path_key(path)
        if key in seen:
            continue
        seen.add(key)
        unique.append(path)
    return unique


def _split_env_directories(raw_value: str | None) -> list[Path]:
    if not raw_value:
        return []

    separators = (";", os.pathsep)
    normalized = raw_value
    for separator in separators:
        if separator != ";":
            normalized = normalized.replace(separator, ";")

    return [Path(chunk.strip()) for chunk in normalized.split(";") if chunk.strip()]


def _candidate_runtime_directories() -> list[Path]:
    candidates: list[Path] = []
    candidates.extend(_split_env_directories(os.environ.get("WEASYPRINT_DLL_DIRECTORIES")))

    msys2_roots: list[Path] = []
    configured_root = os.environ.get("MSYS2_ROOT", "").strip()
    if configured_root:
        msys2_roots.append(Path(configured_root))

    msys2_roots.extend(
        [
            Path(r"C:\msys64"),
            Path(r"C:\tools\msys64"),
        ]
    )

    for root in _dedupe_paths(msys2_roots):
        candidates.extend(
            [
                root / "ucrt64" / "bin",
                root / "mingw64" / "bin",
                root / "clang64" / "bin",
            ]
        )

    candidates.extend(
        [
            Path(r"C:\Program Files\GTK3-Runtime Win64\bin"),
            Path(r"C:\Program Files\GTK3-Runtime\bin"),
        ]
    )
    return _dedupe_paths(candidates)


def _register_dll_directory(path: Path) -> None:
    add_dll_directory = getattr(os, "add_dll_directory", None)
    if add_dll_directory is None:
        return

    key = _path_key(path)
    if key in _REGISTERED_DLL_DIRECTORIES:
        return

    try:
        handle = add_dll_directory(str(path))
    except (FileNotFoundError, OSError):
        return

    _DLL_DIRECTORY_HANDLES.append(handle)
    _REGISTERED_DLL_DIRECTORIES.add(key)


def prepare_weasyprint_environment() -> WeasyPrintRuntimeStatus:
    searched_directories = tuple(_candidate_runtime_directories())

    if sys.platform != "win32":
        return WeasyPrintRuntimeStatus(configured_directories=(), searched_directories=searched_directories)

    configured_directories: list[Path] = []
    for directory in searched_directories:
        if not directory.exists() or not directory.is_dir():
            continue
        configured_directories.append(directory)
        _register_dll_directory(directory)

    configured_directories = _dedupe_paths(configured_directories)
    if configured_directories:
        os.environ["WEASYPRINT_DLL_DIRECTORIES"] = ";".join(str(path) for path in configured_directories)

    return WeasyPrintRuntimeStatus(
        configured_directories=tuple(configured_directories),
        searched_directories=searched_directories,
    )


def build_windows_runtime_help(
    error: BaseException,
    status: WeasyPrintRuntimeStatus | None = None,
) -> str:
    runtime_status = status or prepare_weasyprint_environment()

    lines = [
        "WeasyPrint could not load the system libraries required for PDF export on Windows.",
        "",
        f"Technical detail: {error}",
        "",
    ]

    if runtime_status.configured_directories:
        lines.append("Automatically detected DLL directories:")
        lines.extend(f"- {path}" for path in runtime_status.configured_directories)
    else:
        lines.append("No compatible DLL directory was detected automatically.")

    lines.extend(
        [
            "",
            "Recommended fix:",
            "1. Install MSYS2: winget install --id MSYS2.MSYS2 --accept-package-agreements --accept-source-agreements",
            '2. Install Pango: C:\\msys64\\usr\\bin\\bash.exe -lc "pacman -S --needed mingw-w64-ucrt-x86_64-pango"',
            "3. Restart the application.",
            "",
            "If MSYS2 is installed in a different folder, set WEASYPRINT_DLL_DIRECTORIES",
            "to the directory containing libgobject-2.0-0.dll (e.g. C:\\msys64\\ucrt64\\bin).",
            "",
            "HTML export remains available even if PDF export fails.",
        ]
    )
    return "\n".join(lines)
