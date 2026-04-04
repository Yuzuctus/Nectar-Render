from __future__ import annotations

import json
import logging
import tempfile
from pathlib import Path


logger = logging.getLogger(__name__)


def save_json_file(path: Path, payload: dict[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temp_path: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="w",
            encoding="utf-8",
            dir=path.parent,
            prefix=f"{path.stem}.",
            suffix=".tmp",
            delete=False,
        ) as handle:
            json.dump(payload, handle, ensure_ascii=False, indent=2)
            temp_path = Path(handle.name)
        temp_path.replace(path)
    except Exception:
        if temp_path is not None:
            temp_path.unlink(missing_ok=True)
        raise


def load_json_file(
    path: Path,
    *,
    log: logging.Logger | None = None,
) -> dict[str, object]:
    active_logger = log or logger
    if not path.exists():
        return {}
    try:
        content = json.loads(path.read_text(encoding="utf-8"))
        if isinstance(content, dict):
            return content
    except (json.JSONDecodeError, OSError, UnicodeDecodeError) as exc:
        active_logger.warning(
            "Failed to load JSON file %s (%s): %s",
            path,
            exc.__class__.__name__,
            exc,
        )
        return {}
    return {}


__all__ = ["load_json_file", "save_json_file"]
