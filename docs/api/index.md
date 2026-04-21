# API reference

Every symbol in the `olgadoc` package is documented here. The pages are
generated directly from the live docstrings via `mkdocstrings`, so they
stay in sync with the code.

## Top-level classes

- [`Document`](./document.md) — the primary entry point. Open, inspect,
  extract.
- [`Page`](./page.md) — a single page inside a document.
- [`Processability`](./processability.md) — health report for an opened
  document.

## Exceptions

::: olgadoc.OlgaError
    options:
      show_source: false

## TypedDict payloads

Every method that returns a dict returns a real `TypedDict` — see:

- [Payloads](./payloads.md) — `Link`, `Table`, `SearchHit`, `Chunk`,
  `OutlineEntry`, `ExtractedImage`, `HealthIssue` and friends.
- [JSON tree](./json_tree.md) — `DocumentJson` and the 16-variant
  `JsonElement` discriminated union returned by
  [`Document.to_json`](./document.md#olgadoc.Document.to_json).

## Module version

::: olgadoc.__version__
    options:
      show_source: false
