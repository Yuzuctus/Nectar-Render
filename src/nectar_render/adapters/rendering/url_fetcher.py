from __future__ import annotations

from collections.abc import Callable
from pathlib import Path
from typing import Any
from urllib.parse import unquote, urlparse


_ALLOWED_EXTERNAL_HTTPS_HOSTS = {
    "fonts.googleapis.com",
    "fonts.gstatic.com",
}


def _is_allowed_external_https_url(url: str) -> bool:
    parsed = urlparse(url)
    if parsed.scheme.lower() != "https":
        return False
    host = (parsed.hostname or "").lower()
    return host in _ALLOWED_EXTERNAL_HTTPS_HOSTS


def _is_within_root(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
        return True
    except ValueError:
        return False


def _file_url_to_path(url: str) -> Path:
    parsed = urlparse(url)
    decoded = unquote(parsed.path)
    if decoded.startswith("/") and len(decoded) >= 3 and decoded[2] == ":":
        decoded = decoded[1:]
    return Path(decoded)


def _resolve_base_root(base_url: Path | str | None) -> Path | None:
    if isinstance(base_url, Path):
        return base_url.resolve()
    if not base_url:
        return None

    parsed = urlparse(str(base_url))
    if parsed.scheme in {"", "file"}:
        if parsed.scheme == "file":
            return _file_url_to_path(str(base_url)).resolve()
        return Path(str(base_url)).resolve()
    return None


def build_safe_url_fetcher(
    *,
    base_url: Path | str | None,
    default_fetcher: Callable[[str], dict[str, Any]],
) -> Callable[[str], dict[str, Any]]:
    base_root = _resolve_base_root(base_url)

    def _safe_fetcher(url: str) -> dict[str, Any]:
        parsed = urlparse(url)
        scheme = parsed.scheme.lower()

        if scheme == "ftp":
            raise ValueError(f"External fetch blocked for URL: {url}")

        if scheme in {"http", "https"}:
            if _is_allowed_external_https_url(url):
                return default_fetcher(url)
            raise ValueError(f"External fetch blocked for URL: {url}")

        if scheme == "data":
            return default_fetcher(url)

        if scheme == "file":
            target_path = _file_url_to_path(url).resolve()
            if base_root is None:
                raise ValueError(f"File URL blocked without base root: {url}")
            if not _is_within_root(target_path, base_root):
                raise ValueError(f"Blocked file URL outside base root: {url}")
            return default_fetcher(target_path.as_uri())

        if scheme == "":
            relative_path = Path(unquote(parsed.path or url))
            if base_root is None:
                raise ValueError(f"Relative resource blocked without base root: {url}")
            target_path = (base_root / relative_path).resolve()
            if not _is_within_root(target_path, base_root):
                raise ValueError(f"Relative resource escapes base root: {url}")
            return default_fetcher(target_path.as_uri())

        raise ValueError(f"Unsupported URL scheme '{scheme}' for resource: {url}")

    return _safe_fetcher


__all__ = ["build_safe_url_fetcher"]
