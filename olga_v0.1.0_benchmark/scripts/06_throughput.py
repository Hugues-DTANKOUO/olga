"""
06_throughput.py

Simulates a realistic mixed-corpus ingestion: 50 files of varying formats
processed sequentially in a single Python process. Compares:

  (A) olgadoc as a single unified API for all formats
  (B) a hand-routed "best-of-breed" pipeline (pymupdf + calamine + bs4 + docx2txt)

Reports wall-clock time and total characters extracted.

Usage:  python3 06_throughput.py
"""
import os
import pathlib
import shutil
import tempfile
import time

ROOT = pathlib.Path(__file__).parent.parent
FIXTURES = ROOT / "fixtures"
RESULTS  = ROOT / "results"
RESULTS.mkdir(exist_ok=True)

SOURCES = [
    FIXTURES / "weird_invoice.pdf",
    FIXTURES / "complex.xlsx",
    FIXTURES / "stress.xlsx",
    FIXTURES / "complex.html",
    FIXTURES / "complex.docx",
]
N = 50   # corpus size (10 copies of each of the 5 source fixtures)


def build_corpus(root: pathlib.Path) -> list[pathlib.Path]:
    paths: list[pathlib.Path] = []
    for i in range(N):
        src = SOURCES[i % len(SOURCES)]
        dst = root / f"doc_{i:03d}{src.suffix}"
        shutil.copy(src, dst)
        paths.append(dst)
    return paths


def process_all_olgadoc(files: list[pathlib.Path]) -> int:
    import olgadoc
    total = 0
    for f in files:
        total += len(olgadoc.Document.open(str(f)).text())
    return total


def process_all_best_of_breed(files: list[pathlib.Path]) -> int:
    """Route each file to the 'best' extractor for its format."""
    import fitz
    from python_calamine import CalamineWorkbook
    from bs4 import BeautifulSoup
    import docx2txt

    total = 0
    for f in files:
        ext = f.suffix.lower()
        if ext == ".pdf":
            d = fitz.open(str(f))
            txt = "\n".join(p.get_text() for p in d)
        elif ext == ".xlsx":
            wb = CalamineWorkbook.from_path(str(f))
            parts = []
            for n in wb.sheet_names:
                for row in wb.get_sheet_by_name(n).to_python():
                    parts.append("\t".join("" if v is None else str(v) for v in row))
            txt = "\n".join(parts)
        elif ext == ".html":
            with open(f, encoding="utf-8") as fh:
                txt = BeautifulSoup(fh, "lxml").get_text("\n", strip=True)
        elif ext == ".docx":
            txt = docx2txt.process(str(f))
        else:
            txt = ""
        total += len(txt)
    return total


def main() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        corpus = build_corpus(pathlib.Path(tmp))
        print(f"Corpus: {len(corpus)} files, mixed formats")

        # Warm both pipelines once (loads imports, page caches, etc.)
        process_all_olgadoc(corpus)
        process_all_best_of_breed(corpus)

        t0 = time.perf_counter()
        c1 = process_all_olgadoc(corpus)
        t1 = time.perf_counter()
        c2 = process_all_best_of_breed(corpus)
        t2 = time.perf_counter()

    olga_ms  = (t1 - t0) * 1000
    multi_ms = (t2 - t1) * 1000
    print(f"\n  olgadoc (unified API) : {olga_ms:8.1f} ms   total chars {c1:>7}")
    print(f"  per-format best-of-*  : {multi_ms:8.1f} ms   total chars {c2:>7}")
    print(f"\n  Speedup  : {multi_ms / olga_ms:.2f}×")
    print(f"  Content  : {c1 / c2:.2f}× more characters from olgadoc")

    import csv
    with (RESULTS / "throughput.csv").open("w", newline="") as fh:
        w = csv.writer(fh)
        w.writerow(["pipeline", "total_ms", "total_chars"])
        w.writerow(["olgadoc",        f"{olga_ms:.1f}",  c1])
        w.writerow(["best-of-breed",  f"{multi_ms:.1f}", c2])
    print(f"\n→ wrote {RESULTS / 'throughput.csv'}")


if __name__ == "__main__":
    main()
