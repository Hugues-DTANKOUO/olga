#!/usr/bin/env python3
"""Extract table structure from a PDF using pdfplumber and write JSON output.

Usage:
    python pdfplumber_extract.py <input.pdf> <output.json>

Output format (JSON):
    {
        "file": "input.pdf",
        "pages": [
            {
                "page_number": 0,
                "tables": [
                    {
                        "bbox": [x0, y0, x1, y1],
                        "rows": 3,
                        "cols": 2,
                        "cells": [
                            {"row": 0, "col": 0, "text": "Name"},
                            {"row": 0, "col": 1, "text": "Age"},
                            ...
                        ]
                    }
                ]
            }
        ]
    }

Dependencies:
    pip install pdfplumber
"""

import json
import sys
from pathlib import Path

try:
    import pdfplumber
except ImportError:
    print("Error: pdfplumber not installed. Run: pip install pdfplumber", file=sys.stderr)
    sys.exit(1)


def extract_tables(pdf_path: str) -> dict:
    """Extract table data from all pages of a PDF."""
    result = {
        "file": Path(pdf_path).name,
        "pages": [],
    }

    with pdfplumber.open(pdf_path) as pdf:
        page_width = float(pdf.pages[0].width) if pdf.pages else 1.0
        page_height = float(pdf.pages[0].height) if pdf.pages else 1.0

        for page_idx, page in enumerate(pdf.pages):
            pw = float(page.width)
            ph = float(page.height)

            page_data = {
                "page_number": page_idx,
                "width": pw,
                "height": ph,
                "tables": [],
            }

            tables = page.find_tables()
            for table in tables:
                bbox = table.bbox  # (x0, y0, x1, y1) in PDF points
                # Normalize to [0, 1] coordinates.
                norm_bbox = [
                    bbox[0] / pw,
                    bbox[1] / ph,
                    bbox[2] / pw,
                    bbox[3] / ph,
                ]

                extracted = table.extract()
                if not extracted:
                    continue

                n_rows = len(extracted)
                n_cols = max(len(row) for row in extracted) if extracted else 0

                cells = []
                for r, row in enumerate(extracted):
                    for c, cell_text in enumerate(row):
                        cells.append({
                            "row": r,
                            "col": c,
                            "text": (cell_text or "").strip(),
                        })

                page_data["tables"].append({
                    "bbox": norm_bbox,
                    "rows": n_rows,
                    "cols": n_cols,
                    "cells": cells,
                })

            result["pages"].append(page_data)

    return result


def main():
    if len(sys.argv) != 3:
        print(f"Usage: {sys.argv[0]} <input.pdf> <output.json>", file=sys.stderr)
        sys.exit(1)

    pdf_path = sys.argv[1]
    output_path = sys.argv[2]

    if not Path(pdf_path).exists():
        print(f"Error: PDF not found: {pdf_path}", file=sys.stderr)
        sys.exit(1)

    data = extract_tables(pdf_path)

    with open(output_path, "w") as f:
        json.dump(data, f, indent=2)

    # Summary.
    total_tables = sum(len(p["tables"]) for p in data["pages"])
    total_cells = sum(
        len(t["cells"]) for p in data["pages"] for t in p["tables"]
    )
    print(f"Extracted {total_tables} tables, {total_cells} cells from {pdf_path}")


if __name__ == "__main__":
    main()
