from __future__ import annotations

import logging
from collections.abc import Sequence

from .cli import build_parser, run_cli
from .utils.logging import configure_logging
from .utils.weasyprint_runtime import prepare_weasyprint_environment


logger = logging.getLogger(__name__)


def _run_gui() -> int:
    import tkinter as tk

    from .interfaces.desktop.app import NectarRenderApp

    configure_logging()
    runtime_status = prepare_weasyprint_environment()
    if runtime_status.configured_directories:
        logger.info(
            "WeasyPrint DLL directories configured: %s",
            "; ".join(str(path) for path in runtime_status.configured_directories),
        )
    root = tk.Tk()
    NectarRenderApp(root)
    root.mainloop()
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(list(argv) if argv is not None else None)

    if args.input is not None:
        return run_cli(args)

    return _run_gui()


if __name__ == "__main__":
    raise SystemExit(main())
