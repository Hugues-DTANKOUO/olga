# Batch processability

Walks a directory tree recursively, calls
[`Document.processability()`](../api/processability.md) on every
supported file, and prints a one-line verdict per document. With
`--json`, emits one JSON object per line — perfect for `jq` or a
downstream ingestion pipeline.

Exits non-zero when any document is blocked or errors out, so this
doubles as a pre-flight gate in CI.

## Usage

```bash
python examples/batch_processability.py ROOT          # pretty text
python examples/batch_processability.py ROOT --json   # JSONL output
```

## Source

```python
--8<-- "olgadoc/examples/batch_processability.py"
```
