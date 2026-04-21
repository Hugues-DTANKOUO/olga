"""Quickstart — open a document and print a one-line summary per page.

Usage:
    python examples/quickstart.py PATH [PATH ...]

Each ``PATH`` can be any supported document (PDF, DOCX, XLSX, HTML). The
script opens the document, prints the detected format and page count, and
shows the first 60 characters of every page's extracted text. It is meant
as a literal "hello world" for the library — copy the body into a REPL to
start exploring.
"""

from __future__ import annotations

import sys
from pathlib import Path

import olgadoc


def summarise(path: Path) -> int:
    """Print a compact summary of ``path``. Returns ``0`` on success."""
    try:
        doc = olgadoc.Document.open(str(path))
    except olgadoc.OlgaError as exc:
        print(f"[{path.name}] failed to open: {exc}", file=sys.stderr)
        return 1

    print(f"=== {path.name} ===")
    print(f"  format      : {doc.format}")
    print(f"  page_count  : {doc.page_count}")
    print(f"  file_size   : {doc.file_size} bytes")
    print(f"  title       : {doc.title or '(none)'}")
    print(f"  encrypted   : {doc.encrypted}")

    for page in doc.pages():
        preview = page.text().strip().replace("\n", " ")
        if len(preview) > 60:
            preview = preview[:57] + "..."
        print(f"  page {page.number:>3}  : {preview or '(empty)'}")
    return 0


def main(argv: list[str]) -> int:
    """CLI entry point. Returns a process exit code."""
    if len(argv) < 2:
        print(__doc__, file=sys.stderr)
        return 2
    return max(summarise(Path(arg)) for arg in argv[1:])


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
