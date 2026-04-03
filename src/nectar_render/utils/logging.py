from __future__ import annotations

import logging
from logging.handlers import RotatingFileHandler

from .paths import application_data_dir


def configure_logging(*, verbose: bool = False) -> None:
    level = logging.DEBUG if verbose else logging.INFO
    fmt = "%(asctime)s [%(levelname)s] %(name)s: %(message)s"
    root_logger = logging.getLogger()
    if not root_logger.handlers:
        logging.basicConfig(level=level, format=fmt)
    else:
        root_logger.setLevel(level)

    log_dir = application_data_dir() / "logs"
    try:
        log_dir.mkdir(parents=True, exist_ok=True)
        log_file = log_dir / "nectar-render.log"
        for existing in root_logger.handlers:
            if isinstance(existing, RotatingFileHandler) and getattr(
                existing, "baseFilename", ""
            ) == str(log_file):
                return
        handler = RotatingFileHandler(
            log_file,
            maxBytes=2 * 1024 * 1024,  # 2 MB
            backupCount=3,
            encoding="utf-8",
        )
        handler.setLevel(logging.DEBUG)
        handler.setFormatter(logging.Formatter(fmt))
        root_logger.addHandler(handler)
    except OSError:
        pass
