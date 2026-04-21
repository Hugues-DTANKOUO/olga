"""
03_bench.py

Runs the harness against every (file, extractor) pair and writes the results
as CSV to results/performance.csv. Each pair is a fresh Python subprocess
to guarantee isolation.

Usage:  python3 03_bench.py
"""
import csv
import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).parent.parent
FIXTURES = ROOT / "fixtures"
RESULTS  = ROOT / "results"
RESULTS.mkdir(exist_ok=True)
HARNESS  = ROOT / "scripts" / "02_harness.py"

MATRIX = [
    # (fixture filename, [extractors to run])
    ("weird_invoice.pdf", ["olgadoc", "pdfplumber", "pymupdf", "pypdf"]),
    ("rust_book.pdf",     ["olgadoc", "pymupdf", "pypdf"]),  # pdfplumber skipped (~65 s/run × 10)
    ("complex.xlsx",      ["olgadoc", "calamine", "openpyxl"]),
    ("stress.xlsx",       ["olgadoc", "calamine", "openpyxl"]),
    ("complex.html",      ["olgadoc", "bs4", "trafilatura", "html2text"]),
    ("complex.docx",      ["olgadoc", "docx2txt", "mammoth", "python-docx"]),
]


def run(path: pathlib.Path, lib: str) -> dict:
    """Invoke harness as subprocess; return parsed JSON or {'error': ...}."""
    proc = subprocess.run(
        [sys.executable, str(HARNESS), str(path), lib],
        capture_output=True, text=True, timeout=600,
    )
    last_line = proc.stdout.strip().split("\n")[-1] if proc.stdout.strip() else ""
    try:
        return json.loads(last_line)
    except json.JSONDecodeError:
        return {"error": (proc.stderr or last_line)[:200]}


def main() -> None:
    out_csv = RESULTS / "performance.csv"
    with out_csv.open("w", newline="") as fh:
        w = csv.writer(fh)
        w.writerow(["file", "extractor", "best_ms", "median_ms", "mean_ms", "chars", "error"])

        print(f"{'file':<22} {'lib':<14} {'best_ms':>10} {'median':>9} {'chars':>10}")
        print("-" * 72)
        for fname, libs in MATRIX:
            fpath = FIXTURES / fname
            if not fpath.exists():
                print(f"!! missing fixture: {fpath}")
                continue
            for lib in libs:
                r = run(fpath, lib)
                if "error" in r:
                    print(f"{fname:<22} {lib:<14} ERR {r['error'][:40]}")
                    w.writerow([fname, lib, "", "", "", "", r["error"]])
                else:
                    mark = " ★" if lib == "olgadoc" else ""
                    print(f"{fname:<22} {lib:<14} {r['best_ms']:>9.2f}  "
                          f"{r['median_ms']:>8.2f}  {r['chars']:>10}{mark}")
                    w.writerow([fname, lib, f"{r['best_ms']:.3f}",
                                f"{r['median_ms']:.3f}", f"{r['mean_ms']:.3f}",
                                r["chars"], ""])
            print()
    print(f"→ wrote {out_csv}")


if __name__ == "__main__":
    main()
