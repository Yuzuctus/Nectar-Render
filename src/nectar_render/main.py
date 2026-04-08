"""Nectar Render - Markdown to PDF/HTML converter."""

from collections.abc import Sequence

from .cli import build_parser, run_cli


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(list(argv) if argv is not None else None)
    return run_cli(args)


if __name__ == "__main__":
    raise SystemExit(main())
