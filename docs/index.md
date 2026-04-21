# Olga

> **Four formats. One engine. 15–40× faster.**
>
> Spatial fidelity at native speed, across PDF, DOCX, XLSX, and HTML.
> Rust core, strictly-typed Python bindings. No LLM in the loop.

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

The options were either fast but flat, or faithful but slow. So I
built Olga.

## Why Olga

**One API, four formats.** `pdfplumber` + `python-docx` + `openpyxl` +
`BeautifulSoup`, each with its own API, its own bugs, and its own
edge-case quirks. Olga exposes **one** `Document` class across all four
formats — the method you learn on a PDF works identically on an Excel
workbook.

**Native speed.** The Rust engine clocks in at **4–8 ms on a typical
PDF page, 2 ms on a 5-page DOCX, 1–12 ms on XLSX, and 1–5 ms on HTML**.
That's 15–40× faster than the quality-equivalent open-source tool on
every format — numbers independently measured against 10+ baselines on
an 11-document adversarial corpus. See the full methodology and the
raw per-document results in [BENCHMARKS.md](https://github.com/Hugues-DTANKOUO/olga/blob/main/BENCHMARKS.md).
A post-release reproducible audit of v0.1.0, with fixtures, scripts,
and CSV results, lives at [`olga_v0.1.0_benchmark/`](https://github.com/Hugues-DTANKOUO/olga/tree/main/olga_v0.1.0_benchmark)
— **1.62× faster and 2.62× more content** than a hand-routed
best-of-breed pipeline on a 50-file mixed corpus.

**Spatial fidelity, intact.** Tables stay tables. Columns stay columns.
Figure captions stay next to their figures. Page layout, bounding
boxes, and provenance (observed vs. inferred) survive the round-trip
to Markdown or to the typed JSON tree — validated against an
adversarial corpus where single-format tools routinely drop features.

**OCR pre-flight.** `doc.processability()` tells you whether a
document actually carries native text — or whether it's a scanned
image that needs OCR upstream — *before* you pay for downstream
extraction or LLM work. Olga doesn't do OCR today; it's the first
thing it tells you about, so you can route the file accordingly
without burning a dollar.

**Actually typed.** Zero `Any` on the public Python surface. Every
returned dict is a real `TypedDict`. `Document.to_json()` returns a
discriminated union over sixteen element variants, and
`mypy --strict` narrows each branch to exactly one. Your IDE and your
type-checker both know what's inside.

**No LLM in the loop.** Olga reads the native content stream
directly — no vision model, no GPU, no correction pass. That's how
the anti-LLM adversarial test (invisible canaries, rotated text,
deliberate misspellings) comes back byte-exact: no hallucinations, no
"corrected" typos, no drift.

## Ten-second tour

```python
import olgadoc

doc = olgadoc.Document.open("report.pdf")
print(doc.format, doc.page_count)           # ('PDF', 12)

# Will this document produce text, or does it need OCR first?
report = doc.processability()
if report.is_blocked():
    raise SystemExit(f"blocked by: {[b['kind'] for b in report.blockers]}")

# Full-text search across the whole document
for hit in doc.search("executive summary"):
    print(hit["page"], hit["snippet"])

# Structured JSON tree for downstream indexing
tree = doc.to_json()                        # DocumentJson — fully typed
for element in tree["elements"]:
    if element["type"] == "heading":        # discriminated on "type"
        print(f"h{element['level']}: {element['text']}")
```

## When Olga fits

- **Retrieval-augmented generation.** Chunk documents with provenance
  (page, bbox), search across a corpus, surface hits with citations.
- **Document QA and extraction.** Pull tables out of expense reports,
  read structured data from invoices, analyse contracts.
- **Pipeline gating.** Health-check a directory before paying for
  downstream OCR or LLM work. Fail fast on scanned or corrupt files.
- **Archive migration.** Convert legacy PDF / DOCX archives into
  structured JSON or Markdown for re-indexing.

## When Olga doesn't (yet)

- **Scanned-only PDFs.** Olga reads native content streams, not
  pixels. That's exactly what `processability()` is for — it flags
  `EmptyContent` on the blocker list before you spend money
  downstream, so you can route the file to an OCR step.
- **Rendered-browser HTML semantics.** Olga's HTML extractor follows
  the "literal DOM" school (like `BeautifulSoup`, `pandoc`, search
  indexers) and includes `display:none` content. If you need the
  rendered `outerText` semantics, use a headless browser.
- **Configurable editorial policies.** Today's conventions (`#ERR:`
  prefix for Excel errors, lowercase booleans, escaped intra-cell
  newlines, hidden-sheet inclusion) are fixed by design. Flags to
  toggle them are on the roadmap, not in 0.1.

## Install

```bash
pip install olgadoc
```

One abi3 wheel covers CPython 3.8+.

## Where to go next

- **[Quickstart](./quickstart.md)** — install, open a document, print a
  page, search.
- **[API reference](./api/index.md)** — every class, method, and
  payload, generated from live docstrings.
- **[Examples](./examples/index.md)** — five runnable scripts covering
  the most common pipelines.
- **[Benchmarks](https://github.com/Hugues-DTANKOUO/olga/blob/main/BENCHMARKS.md)**
  — third-party evaluation, full corpus, baselines, known limitations.
- **[Independent v0.1.0 audit](https://github.com/Hugues-DTANKOUO/olga/tree/main/olga_v0.1.0_benchmark)**
  — post-release reproducible benchmark with fixtures, scripts, CSVs.
- **[Changelog](./changelog.md)** — release notes.
- **[Contributing](./contributing.md)** — local development setup.

## License

Olga is distributed under the
[Apache License, Version 2.0](https://github.com/Hugues-DTANKOUO/olga/blob/main/LICENSE).
