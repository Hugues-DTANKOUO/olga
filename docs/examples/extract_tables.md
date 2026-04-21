# Extract tables

Opens a document, pulls every reconstructed table out via
`Document.tables()`, and prints each one as a tab-separated grid. Notes
when a table spans multiple pages via the `is_cross_page` flag on the
[`Table`](../api/payloads.md) payload.

## Usage

```bash
python examples/extract_tables.py PATH
```

## Source

```python
--8<-- "olgadoc/examples/extract_tables.py"
```
