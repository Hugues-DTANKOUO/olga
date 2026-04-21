# Olga

> **Four formats. One engine. 15–40× faster.**
>
> Spatial fidelity at native speed, across PDF, DOCX, XLSX, and HTML.
> Rust core, strictly-typed Python bindings. No LLM in the loop.

[![CI](https://github.com/Hugues-DTANKOUO/olga/actions/workflows/ci.yml/badge.svg)](https://github.com/Hugues-DTANKOUO/olga/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/olga.svg)](https://crates.io/crates/olga)
[![PyPI](https://img.shields.io/pypi/v/olgadoc.svg)](https://pypi.org/project/olgadoc/)
[![License: Apache 2.0](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](https://github.com/Hugues-DTANKOUO/olga/blob/main/LICENSE)
[![Rust 1.88+](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](https://www.rust-lang.org/)
[![Python 3.8+](https://img.shields.io/badge/python-3.8%2B-blue.svg)](https://www.python.org/)

---

## The bet

I waited almost a year for one tool to handle the four formats that
make up most of the world's documents — PDF, DOCX, XLSX, HTML — at
native speed, **without sacrificing spatial fidelity**. A table that
reads across a page is not the same document as its cells flattened
into a line of commas. A figure caption pinned to an image is not the
same caption once it has drifted ten paragraphs away. Layout carries
meaning; lose it and every downstream stage — RAG, LLM extraction,
analytics — quietly degrades. Garbage in, garbage out, no matter how
big the model.

The options were either fast but flat, or faithful but slow. The fast
ones handed back a wall of text. The faithful ones took 75–170 ms a
page and only covered one format. So I built Olga.

## Install

```bash
# Python
pip install olgadoc

# Rust
cargo add olga
```

## Ten-second tour

```python
import olgadoc

doc = olgadoc.Document.open("report.pdf")
print(doc.format, doc.page_count)            # ('PDF', 12)

# Will this document produce text, or does it need OCR first?
report = doc.processability()
if report.is_blocked():
    raise SystemExit([b["kind"] for b in report.blockers])

# Full-text search across the whole document
for hit in doc.search("executive summary"):
    print(hit["page"], hit["snippet"])

# Typed JSON tree for downstream indexing
for element in doc.to_json()["elements"]:
    if element["type"] == "heading":          # discriminated on "type"
        print(f"h{element['level']}: {element['text']}")
```

## Why Olga

| | |
| :--- | :--- |
| **One API, four formats** | `pdfplumber` + `python-docx` + `openpyxl` + `BeautifulSoup` — four libraries, four APIs, four sets of edge cases. Olga exposes a single `Document` class. The method you learn on a PDF works identically on a spreadsheet. |
| **Native speed** | **PDF 4–8 ms · DOCX 2 ms · XLSX 1–12 ms · HTML 1–5 ms.** 15–40× faster than the quality-equivalent open-source tool on every format. ([benchmarks](./BENCHMARKS.md)) |
| **Spatial fidelity, intact** | Tables stay tables. Columns stay columns. Figure captions stay next to their figures. Page layout, bounding boxes, and provenance survive the round-trip to Markdown or to the typed JSON tree. |
| **OCR pre-flight** | `doc.processability()` tells you — before the pipeline starts — whether a document actually carries native text, or whether it's a scanned image that needs OCR first. Fail fast, save money. |
| **Actually typed** | Zero `Any` on the public Python surface. Every returned dict is a real `TypedDict`. `Document.to_json()` returns a discriminated union over 16 element variants — `mypy --strict` narrows each branch to exactly one. |
| **No LLM in the loop** | Reads the native content stream directly. Independently validated with an anti-LLM adversarial test — invisible canaries preserved byte-exact, deliberate typos intact, no hallucinations. |

## Speed, measured

Third-party evaluation against 10+ open-source baselines on an
11-document adversarial corpus:

| Format | Olga | Best quality-equivalent OSS | Speedup |
| :--- | ---: | :--- | ---: |
| PDF (1 page dense) | **8 ms** | `pdftotext -layout` — 75 ms | **9×** |
| DOCX (5 pages) | **2 ms** | `pandoc` — 170 ms | **85×** |
| XLSX (500 rows × 5 sheets) | **12 ms** | `pandas.read_excel` — 35–50 ms | **3–4×** |
| HTML (form + tables) | **1–5 ms** | `pandoc` — 76 ms | **15–80×** |

Quality reaches **100 / 100 on a 6-dimension scoring harness** for
typical PDF, DOCX, HTML, and XLSX documents — matching the best
open-source tool in every format, with consistent cross-format
editorial conventions that no single OSS tool provides.

**Full methodology, corpus, baselines, and known limitations:
[BENCHMARKS.md](./BENCHMARKS.md)**

A post-release independent audit of v0.1.0 — fully reproducible, with
fixtures, scripts, and raw CSVs — lives at
[`olga_v0.1.0_benchmark/`](./olga_v0.1.0_benchmark/). On a 50-file
mixed-format corpus, olgadoc is **1.62× faster and extracts 2.62× more
content** than a hand-routed best-of-breed pipeline.

## vs alternatives

| | Olga | `pdfplumber` | `unstructured` | `docling` | Apache Tika |
| :--- | :-: | :-: | :-: | :-: | :-: |
| PDF | ✅ | ✅ | ✅ | ✅ | ✅ |
| DOCX | ✅ | — | ✅ | ✅ | ✅ |
| XLSX | ✅ | — | partial | partial | ✅ |
| HTML | ✅ | — | ✅ | partial | ✅ |
| One unified `Document` API | ✅ | — | ✅ | ✅ | ✅ |
| Strictly-typed Python (no `Any`) | ✅ | — | — | — | — |
| OCR pre-flight (`processability`) | ✅ | — | — | — | — |
| Provenance per element (observed vs inferred) | ✅ | — | — | — | — |
| Native-code speed | ✅ | — | — | — | JVM |
| No LLM / no GPU required | ✅ | ✅ | optional | optional | ✅ |

## When Olga fits

- **Retrieval-augmented generation.** Chunk documents with provenance
  (page, bbox), search across a corpus, surface hits with citations.
- **Document QA and extraction.** Pull tables out of expense reports,
  read structured data from invoices, analyse contracts.
- **Pipeline gating.** Health-check a directory before paying for
  downstream OCR or LLM work; fail fast on scanned or corrupt files.
- **Archive migration.** Convert legacy PDF / DOCX archives into
  structured JSON or Markdown for re-indexing.

## When Olga doesn't (yet)

- **Scanned-only PDFs.** Olga reads the native content stream, not
  pixels. That's exactly what `processability()` is for: it flags
  `EmptyContent` on the blocker list before you spend money
  downstream, so you can route the file to an OCR step.
- **Rendered-browser HTML.** Olga's HTML extractor follows the
  "literal DOM" school (like BeautifulSoup, pandoc, search indexers)
  and includes `display:none` content. If you need the rendered
  `outerText` semantics, use a headless browser.
- **Configurable editorial policies.** Today's conventions (`#ERR:`
  prefix for Excel errors, lowercase booleans, escaped intra-cell
  newlines, hidden-sheet inclusion) are fixed by design. Flags to
  toggle them are on the roadmap, not in 0.1.

## CLI

```bash
olga report.pdf                        # extract (default: structured JSON)
olga report.pdf --format markdown      # or Markdown / plain text
olga inspect report.pdf                # metadata, page count, health report
olga search  report.pdf "revenue"      # case-insensitive full-text search
olga pages   report.pdf --page 3       # one specific page
```

`olga --help` and `olga <subcommand> --help` list every flag.

## What's in the box

```
olga/
├─ src/                  # Rust engine (PDF / DOCX / XLSX / HTML)
├─ tests/                # Cargo integration tests + corpus fixtures
├─ benches/              # Criterion benchmarks
├─ olgadoc/              # Python bindings (PyO3 + maturin)
│  ├─ python/olgadoc/    # Python package: TypedDicts, .pyi stub
│  ├─ tests/             # pytest suite
│  └─ examples/          # Five runnable scripts
├─ docs/                 # MkDocs site (mkdocstrings → live API)
├─ docs-dev/             # Internal architecture notes & ADRs
└─ .github/workflows/    # CI: rust + python + wheels + docs
```

## Documentation

- **[Quickstart](./docs/quickstart.md)** — install, open, extract, search.
- **[Benchmarks](./BENCHMARKS.md)** — third-party evaluation, full
  corpus, baselines, known limitations.
- **[Independent v0.1.0 benchmark](./olga_v0.1.0_benchmark/)** —
  post-release reproducible audit with fixtures, scripts, and CSV
  results.
- **[API reference](./docs/api/index.md)** — every class, method, and
  TypedDict payload, generated from live docstrings.
- **[Examples](./olgadoc/examples/)** — five self-contained scripts:
  quickstart, table extraction, batch processability, search + extract,
  typed JSON walk.
- **[Changelog](./CHANGELOG.md)** — release notes.
- **[Contributing](./CONTRIBUTING.md)** — local dev setup and review
  conventions.

## Built by

I'm a developer who kept hitting the same wall: every document
pipeline I worked on spent the first half of its life stitching
libraries together, and the second half paying an LLM to paper over
what those libraries had mangled. Olga is the tool I wanted to exist
— so I wrote it. Questions, bug reports, corpus files that break
things: please open an issue.

— Hugues

## License

Distributed under the [Apache License, Version 2.0](https://github.com/Hugues-DTANKOUO/olga/blob/main/LICENSE).
