"""
05_robustness.py

Feeds each extractor a series of malformed / edge-case inputs and checks
who raises cleanly, who returns garbage, who crashes the interpreter.
Writes results/robustness.csv.

Usage:  python3 05_robustness.py
"""
import csv
import json
import pathlib
import subprocess
import sys
import tempfile

import fitz
from docx import Document

ROOT = pathlib.Path(__file__).parent.parent
FIXTURES = ROOT / "fixtures"
RESULTS  = ROOT / "results"
RESULTS.mkdir(exist_ok=True)
HARNESS = ROOT / "scripts" / "02_harness.py"


def build_edge_cases(tmp: pathlib.Path) -> list[tuple[str, pathlib.Path, list[str]]]:
    """Create four pathological inputs and return (label, path, extractors)."""
    # 1. An empty file with .pdf extension
    (tmp / "empty.pdf").write_bytes(b"")

    # 2. A valid PDF with extensive Unicode and a visual grid
    doc = fitz.open()
    page = doc.new_page()
    page.insert_text((72, 72), "Émoji test 🎉 日本語 العربية русский", fontsize=14)
    for i in range(5):
        y = 120 + i * 30
        page.insert_text((72,  y), f"Row {i}: col1")
        page.insert_text((200, y), f"col2={i * 100}")
        page.insert_text((350, y), "✓" if i % 2 else "✗")
    doc.set_metadata({"title": "Test 测试 тест",
                      "author": "Jörg Müller & co.",
                      "keywords": "unicode,stress,pdf"})
    doc.save(str(tmp / "unicode.pdf"))
    doc.close()

    # 3. Broken HTML: unclosed tags, injected script, numeric entities
    (tmp / "broken.html").write_text(
        """<!DOCTYPE html><html><body>
<h1>Broken HTML test</h1>
<p>Unclosed <strong>bold
<div>and <em>italic without close
<p>Another paragraph <b>with <i>nested <code>unclosed
<table><tr><td>cell1<td>cell2<tr><td>cell3
<script>var x = "</script>injected";</script>
Price: 100€ &euro; &#8364; &amp;<br>
</body></html>""",
        encoding="utf-8",
    )

    # 4. Valid but minimal DOCX (one paragraph)
    d = Document()
    d.add_paragraph("Minimal DOCX with just one line.")
    d.save(str(tmp / "minimal.docx"))

    # 5. Truncated XLSX (ZIP cut in half)
    src = (FIXTURES / "stress.xlsx").read_bytes()
    (tmp / "corrupt.xlsx").write_bytes(src[: len(src) // 2])

    return [
        ("empty.pdf",    tmp / "empty.pdf",    ["olgadoc", "pymupdf", "pdfplumber", "pypdf"]),
        ("unicode.pdf",  tmp / "unicode.pdf",  ["olgadoc", "pymupdf", "pdfplumber", "pypdf"]),
        ("broken.html",  tmp / "broken.html",  ["olgadoc", "bs4", "trafilatura"]),
        ("minimal.docx", tmp / "minimal.docx", ["olgadoc", "docx2txt", "mammoth"]),
        ("corrupt.xlsx", tmp / "corrupt.xlsx", ["olgadoc", "calamine", "openpyxl"]),
    ]


def probe(path: pathlib.Path, lib: str) -> dict:
    proc = subprocess.run(
        [sys.executable, str(HARNESS), str(path), lib],
        capture_output=True, text=True, timeout=60,
    )
    last = proc.stdout.strip().split("\n")[-1] if proc.stdout.strip() else ""
    try:
        return json.loads(last)
    except json.JSONDecodeError:
        return {"error": "interpreter crash: " + (proc.stderr or "")[-150:].strip()}


def main() -> None:
    with tempfile.TemporaryDirectory() as tmp_s:
        tmp = pathlib.Path(tmp_s)
        cases = build_edge_cases(tmp)

        rows = [("file", "extractor", "outcome", "detail")]
        print(f"\n{'file':<16} {'extractor':<14} outcome")
        print("-" * 60)
        for label, path, libs in cases:
            for lib in libs:
                r = probe(path, lib)
                if "error" in r:
                    verdict = "raised" if "crash" not in r["error"] else "CRASH"
                    detail = r["error"][:120]
                else:
                    verdict = "OK"
                    detail = f"{r['chars']} chars"
                print(f"{label:<16} {lib:<14} {verdict:<6}  {detail[:60]}")
                rows.append((label, lib, verdict, detail))
            print()

        with (RESULTS / "robustness.csv").open("w", newline="") as fh:
            csv.writer(fh).writerows(rows)
        print(f"→ wrote {RESULTS / 'robustness.csv'}")


if __name__ == "__main__":
    main()
