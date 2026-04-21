# Search and extract

Runs `Document.search(QUERY)` and prints every hit with its surrounding
page text — a compact demo of how to pair
[`SearchHit`](../api/payloads.md) metadata (page, line, column, match,
snippet) with the page's full text rendering.

## Usage

```bash
python examples/search_and_extract.py PATH QUERY [--context N]
```

`--context` controls how many characters of surrounding page text to
print (default 200).

## Source

```python
--8<-- "olgadoc/examples/search_and_extract.py"
```
