# olgadoc

> **Four formats. One engine. 15–40× faster.**
>
> Spatial fidelity at native speed, across PDF, DOCX, XLSX, and HTML.
> One `Document` API. `mypy --strict` clean. No LLM in the loop.

Python bindings for [Olga](https://github.com/Hugues-DTANKOUO/olga) —
a Rust document-processing engine. Built on [PyO3](https://pyo3.rs)
and [maturin](https://www.maturin.rs/); one abi3 wheel covers
CPython 3.8+.

## Install

```bash
pip install olgadoc
```

## Ten-second tour

```python
import olgadoc

doc = olgadoc.Document.open("report.pdf")
print(doc.format, doc.page_count)           # ('PDF', 12)

# Will this document produce text, or does it need OCR first?
report = doc.processability()
if report.is_blocked():
    raise SystemExit([b["kind"] for b in report.blockers])

# Full-text search
for hit in doc.search("quarterly revenue"):
    print(hit["page"], hit["snippet"])

# Structured JSON tree — discriminated on ``type``
for element in doc.to_json()["elements"]:
    if element["type"] == "heading":
        print(f"h{element['level']}: {element['text']}")
```

## Why olgadoc

- **Four formats, one API.** PDF, DOCX, XLSX, and HTML all expose the
  same `Document` / `Page` surface. Stop juggling `pdfplumber` +
  `python-docx` + `openpyxl` + `BeautifulSoup`.
- **Native speed.** PDF 4–8 ms · DOCX 2 ms · XLSX 1–12 ms · HTML 1–5 ms.
  15–40× faster than the quality-equivalent tool on every format.
  ([benchmarks](https://github.com/Hugues-DTANKOUO/olga/blob/main/BENCHMARKS.md))
- **Spatial fidelity, intact.** Tables stay tables. Columns stay
  columns. Figure captions stay next to their figures. Layout carries
  meaning, and Olga preserves it across the round-trip to Markdown or
  to the typed JSON tree.
- **OCR pre-flight.** `doc.processability()` tells you — before the
  pipeline starts — whether a document actually carries native text,
  or whether it's a scanned image that needs OCR first. Fail fast,
  save money.
- **Actually typed.** Zero `Any` on the public surface. Every returned
  dict is a real `TypedDict`, `Document.to_json()` returns a
  discriminated union over 16 element variants, and `mypy --strict`
  narrows each branch.
- **No LLM in the loop.** Reads the native content stream directly.
  Validated with an anti-LLM adversarial test — invisible canaries
  preserved byte-exact, deliberate typos intact, no hallucinations.

## Typed surface, no `Any`

Every returned dict is a runtime `TypedDict` — introspectable at
runtime and narrowed at type-check time.

```python
from olgadoc import SearchHit

def show(hit: SearchHit) -> None:
    print(hit["page"], hit["snippet"])  # ok
    print(hit["nope"])                  # mypy: "SearchHit" has no key "nope"
```

`Document.to_json()` returns a [`DocumentJson`][DocumentJson] tree
whose `elements` are a discriminated [`JsonElement`][JsonElement]
union over 16 variants (`heading`, `paragraph`, `table`, `list`,
`image`, `code_block`, …). Mypy narrows each branch to exactly one.

[DocumentJson]: https://github.com/Hugues-DTANKOUO/olga/blob/main/docs/api/json_tree.md
[JsonElement]: https://github.com/Hugues-DTANKOUO/olga/blob/main/docs/api/json_tree.md

## vs alternatives

| | olgadoc | `pdfplumber` | `unstructured` | `docling` |
| :--- | :-: | :-: | :-: | :-: |
| PDF | ✅ | ✅ | ✅ | ✅ |
| DOCX | ✅ | — | ✅ | ✅ |
| XLSX | ✅ | — | partial | partial |
| HTML | ✅ | — | ✅ | partial |
| `mypy --strict` clean (no `Any`) | ✅ | — | — | — |
| OCR pre-flight | ✅ | — | — | — |
| Provenance per element | ✅ | — | — | — |
| No ML model / no GPU required | ✅ | ✅ | optional | optional |

## What you get

- **Four formats, one API** — PDF, DOCX, XLSX, HTML through `Document`.
- **Processability report** — `Document.processability()` → blockers
  (including `EmptyContent` for scanned PDFs) and degradations.
- **Cross-page tables** — anchored on the first page with `is_cross_page`.
- **Hyperlinks, images, outline, RAG chunks, case-insensitive search.**
- **Structured JSON tree** — `Document.to_json()`, discriminated union
  over 16 element variants.

## Examples

Five runnable scripts live in
[`examples/`](https://github.com/Hugues-DTANKOUO/olga/tree/main/olgadoc/examples):

- `quickstart.py` — open a document, print a per-page preview.
- `extract_tables.py` — pull every reconstructed table as TSV.
- `batch_processability.py` — recursively health-check a directory.
- `search_and_extract.py` — search + print surrounding page text.
- `json_walk.py` — walk the typed JSON tree and narrow by `type`.

## Building from source

```bash
pip install maturin
cd olgadoc
maturin develop --release
pytest tests/ -q
```

## Links

- **Source & docs** — [github.com/Hugues-DTANKOUO/olga](https://github.com/Hugues-DTANKOUO/olga)
- **Benchmarks** — [BENCHMARKS.md](https://github.com/Hugues-DTANKOUO/olga/blob/main/BENCHMARKS.md)
- **API reference** — [hugues-dtankouo.github.io/olga](https://hugues-dtankouo.github.io/olga/api/)

## License

[Apache License 2.0](https://github.com/Hugues-DTANKOUO/olga/blob/main/LICENSE).
