# JSON walk

Walks the discriminated [`JsonElement`](../api/json_tree.md) tree
returned by `Document.to_json()` and prints a structural summary —
headings with their level, tables with their dimensions, lists with
their ordering, images with their alt-text, and so on.

Each branch of the walker narrows the union on `element["type"]`, so
`mypy --strict` proves the downstream indexed accesses are safe.

This is the canonical starting point for RAG indexing, structural
diffs, or any other pipeline that needs to reason about document
structure rather than just raw text.

## Usage

```bash
python examples/json_walk.py PATH [--depth N]
```

## Source

```python
--8<-- "olgadoc/examples/json_walk.py"
```
