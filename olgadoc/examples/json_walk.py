"""Walk a document's JSON tree and print a typed structural summary.

Usage:
    python examples/json_walk.py PATH [--depth N]

Calls :meth:`Document.to_json` and walks the resulting :data:`JsonElement`
tree, showing the hierarchy of element kinds up to ``--depth`` levels
(default: unlimited). Headings print their level and text, tables print
their shape and first header row, images print their alt-text, and so on.

This is the canonical example for anyone building a RAG index, a
structural diff, or any downstream tool that needs to reason about
document structure rather than raw text.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import olgadoc
from olgadoc import JsonElement


def _describe(element: JsonElement) -> str:
    """Render a one-line summary of an element, using its discriminated type."""
    # Each branch narrows the union to a single TypedDict variant, so the
    # indexed access below is type-safe.
    if element["type"] == "heading":
        return f"heading h{element['level']}: {element['text']!r}"
    if element["type"] == "paragraph":
        preview = element["text"]
        if len(preview) > 60:
            preview = preview[:57] + "..."
        return f"paragraph: {preview!r}"
    if element["type"] == "table":
        headers = element["headers"]
        headers_txt = ", ".join(headers[:4]) + ("..." if len(headers) > 4 else "")
        return f"table {element['rows']}x{element['cols']} headers=[{headers_txt}]"
    if element["type"] == "list":
        return f"list ordered={element['ordered']}"
    if element["type"] == "list_item":
        return f"list_item: {element['text']!r}"
    if element["type"] == "image":
        alt = element["alt_text"] or "(no alt)"
        return f"image {element['format']}: {alt!r}"
    if element["type"] == "code_block":
        lang = element["language"] or "plain"
        return f"code_block ({lang})"
    if element["type"] == "block_quote":
        return f"block_quote: {element['text'][:40]!r}"
    if element["type"] == "footnote":
        return f"footnote {element['footnote_id']}: {element['text'][:40]!r}"
    if element["type"] == "aligned_line":
        return f"aligned_line: {element['text'][:40]!r}"
    if element["type"] == "page_header":
        return f"page_header: {element['text']!r}"
    if element["type"] == "page_footer":
        return f"page_footer: {element['text']!r}"
    if element["type"] == "section":
        return f"section level={element['level']}"
    # ``document``, ``table_row``, ``table_cell`` — containers with no text.
    return element["type"]


def _walk(element: JsonElement, depth: int, max_depth: int | None) -> None:
    """Print ``element`` and recurse into its children up to ``max_depth``."""
    indent = "  " * depth
    print(f"{indent}- {_describe(element)}")
    if max_depth is not None and depth >= max_depth:
        return
    for child in element.get("children", []):
        _walk(child, depth + 1, max_depth)


def main(argv: list[str]) -> int:
    """CLI entry point. Returns a process exit code."""
    parser = argparse.ArgumentParser(
        description="Walk the JSON tree of a document and print a typed summary.",
    )
    parser.add_argument("path", type=Path, help="document to walk")
    parser.add_argument(
        "--depth",
        type=int,
        default=None,
        help="max recursion depth (default: unlimited)",
    )
    args = parser.parse_args(argv[1:])

    try:
        doc = olgadoc.Document.open(str(args.path))
    except olgadoc.OlgaError as exc:
        print(f"failed to open {args.path}: {exc}", file=sys.stderr)
        return 1

    payload = doc.to_json()
    print(f"{args.path.name}  [{payload['source']['format']}]")
    print(f"  olga_version : {payload['olga_version']}")
    print(f"  pages        : {len(payload['pages'])}")
    print(f"  elements     : {len(payload['elements'])}")
    print()
    for element in payload["elements"]:
        _walk(element, depth=0, max_depth=args.depth)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
