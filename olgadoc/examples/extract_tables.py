"""Extract tables from a document and print them as CSV-style rows.

Usage:
    python examples/extract_tables.py PATH

For each reconstructed table, prints the source page, table id, column
count, then a tab-separated header row followed by tab-separated data
rows. Demonstrates the :class:`olgadoc.Table` payload — including the
``is_cross_page`` flag for tables that span multiple pages.
"""

from __future__ import annotations

import csv
import sys
from pathlib import Path

import olgadoc
from olgadoc import Table


def _as_tsv(rows: list[list[str]]) -> str:
    """Render a table grid as one TSV blob — ``csv`` keeps escaping sane."""
    import io

    buf = io.StringIO()
    writer = csv.writer(buf, delimiter="\t", quoting=csv.QUOTE_MINIMAL)
    writer.writerows(rows)
    return buf.getvalue().rstrip("\n")


def _grid_from_cells(table: Table) -> list[list[str]]:
    """Reconstruct a dense [rows][cols] grid from the ``cells`` payload."""
    rows, cols = table["rows"], table["cols"]
    grid: list[list[str]] = [["" for _ in range(cols)] for _ in range(rows)]
    for cell in table["cells"]:
        if 0 <= cell["row"] < rows and 0 <= cell["col"] < cols:
            grid[cell["row"]][cell["col"]] = cell["text"]
    return grid


def render(table: Table, index: int) -> None:
    """Pretty-print a single :class:`Table` payload."""
    flag = " (cross-page)" if table.get("is_cross_page") else ""
    page_span = (
        f"page {table['first_page']}"
        if table["first_page"] == table["last_page"]
        else f"pages {table['first_page']}-{table['last_page']}"
    )
    print(
        f"--- table #{index} on {page_span}"
        f" — {table['rows']} x {table['cols']}{flag} ---"
    )
    grid = _grid_from_cells(table)
    if grid:
        print(_as_tsv(grid))
    else:
        print("(empty table)")
    print()


def main(argv: list[str]) -> int:
    """CLI entry point. Returns a process exit code."""
    if len(argv) != 2:
        print(__doc__, file=sys.stderr)
        return 2
    path = Path(argv[1])
    try:
        doc = olgadoc.Document.open(str(path))
    except olgadoc.OlgaError as exc:
        print(f"failed to open {path}: {exc}", file=sys.stderr)
        return 1

    tables = doc.tables()
    if not tables:
        print(f"no tables in {path.name}")
        return 0

    print(f"{len(tables)} table(s) in {path.name}\n")
    for index, table in enumerate(tables, start=1):
        render(table, index)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
