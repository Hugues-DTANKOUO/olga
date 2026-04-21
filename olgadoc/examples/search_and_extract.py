"""Search a document and print every hit with its surrounding page text.

Usage:
    python examples/search_and_extract.py PATH QUERY [--context N]

Opens ``PATH``, runs :meth:`Document.search` with ``QUERY`` and prints one
block per hit — the page number, the snippet the engine returned, and the
first ``N`` characters of the page's surrounding text (default 200).

This example demonstrates three things side-by-side: the high-level
:meth:`Document.search` API, the per-page :meth:`Page.text` rendering,
and the :class:`SearchHit` payload's ``line`` / ``col_start`` / ``match``
/ ``snippet`` fields.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import olgadoc


def main(argv: list[str]) -> int:
    """CLI entry point. Returns a process exit code."""
    parser = argparse.ArgumentParser(
        description="Search a document and print surrounding text per hit.",
    )
    parser.add_argument("path", type=Path, help="document to search")
    parser.add_argument("query", help="substring to look for (case-insensitive)")
    parser.add_argument(
        "--context",
        type=int,
        default=200,
        help="characters of surrounding page text to print (default: 200)",
    )
    args = parser.parse_args(argv[1:])

    try:
        doc = olgadoc.Document.open(str(args.path))
    except olgadoc.OlgaError as exc:
        print(f"failed to open {args.path}: {exc}", file=sys.stderr)
        return 1

    hits = doc.search(args.query)
    if not hits:
        print(f"no hits for {args.query!r} in {args.path.name}")
        return 0

    print(f"{len(hits)} hit(s) for {args.query!r} in {args.path.name}\n")
    for idx, hit in enumerate(hits, start=1):
        page = doc.page(hit["page"])
        if page is None:
            continue
        text = page.text()
        context = text[: args.context].replace("\n", " ")
        if len(text) > args.context:
            context += "..."
        print(
            f"[{idx}] page {hit['page']}"
            f" line {hit['line']} col {hit['col_start']}"
            f" match={hit['match']!r}"
        )
        print(f"    snippet: {hit['snippet']}")
        print(f"    context: {context}")
        print()
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
