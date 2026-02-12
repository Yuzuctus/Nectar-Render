from __future__ import annotations

import logging
import tkinter as tk

from .ui.app import MarkdownToPdfApp
from .utils.logging import configure_logging
from .utils.weasyprint_runtime import prepare_weasyprint_environment


logger = logging.getLogger(__name__)


def main() -> None:
    configure_logging()
    runtime_status = prepare_weasyprint_environment()
    if runtime_status.configured_directories:
        logger.info(
            "WeasyPrint DLL directories configured: %s",
            "; ".join(str(path) for path in runtime_status.configured_directories),
        )
    root = tk.Tk()
    MarkdownToPdfApp(root)
    root.mainloop()


if __name__ == "__main__":
    main()
