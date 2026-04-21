# Quickstart

## Install

Olga ships as a pre-built wheel on PyPI:

```bash
pip install olgadoc
```

One wheel covers every CPython 3.8+ (abi3).

If you'd rather build from source, clone the repo and run:

```bash
cd olgadoc
maturin develop --release
```

## Open a document

```python
import olgadoc

doc = olgadoc.Document.open("report.pdf")
print(f"{doc.format} — {doc.page_count} pages, {doc.file_size} bytes")
```

`Document.open` handles PDF, DOCX, XLSX, and HTML transparently. Pass a
`bytes` payload with `Document.open_bytes(data, format="pdf")` when the
document is already in memory.

## Inspect a page

```python
first = doc.page(1)
print(first.text()[:200])             # plain text
print(first.markdown()[:200])         # GitHub-flavoured markdown
print(len(first.tables()), "tables")  # per-page tables
```

Pages are 1-indexed. `doc.pages()` returns them all in order.

## Health-check before processing

Olga doesn't do OCR. That's exactly what `processability()` is for: it
tells you, before the rest of your pipeline starts spending money,
whether the document actually carries native, extractable text.

```python
report = doc.processability()
if report.is_blocked():
    raise SystemExit(f"blocked: {[b['kind'] for b in report.blockers]}")
if report.is_degraded():
    print("warnings:", [d["kind"] for d in report.degradations])
```

**Blockers** mean Olga has no content to work with — route the file
elsewhere:

- `EmptyContent` — a scanned PDF (or any document with no native
  text). Send it through OCR first, then re-open the result.
- `Encrypted` — password-protected input. Decrypt, then re-open.
- `DecodeFailed` — the source is malformed or truncated.

**Degradations** mean Olga can still produce output but fidelity
drops on specific axes (heuristic structure, approximate pagination,
partial extraction). Surface the counts if they matter to your
pipeline; otherwise, keep going.

## Full-text search

```python
for hit in doc.search("quarterly revenue"):
    print(f"page {hit['page']} line {hit['line']}: {hit['snippet']}")
```

Matches are case-insensitive substring matches. `Page.search` runs the
same query on a single page.

## Structured JSON tree

For anything more structural — RAG indexing, document diffing,
template extraction — use `to_json()`:

```python
payload = doc.to_json()
for element in payload["elements"]:
    if element["type"] == "table":
        print(f"table {element['rows']}x{element['cols']}")
    elif element["type"] == "heading":
        print(f"h{element['level']}: {element['text']}")
```

The top-level shape is `DocumentJson` and every node is one of sixteen
variants in the `JsonElement` union — see the
[JSON tree reference](./api/json_tree.md) for the full schema. Mypy
narrows each branch to the exact variant its `type` literal identifies.

## Next steps

- Browse the [API reference](./api/index.md) for every type and method.
- Read the [examples](./examples/index.md) for five complete scripts.
