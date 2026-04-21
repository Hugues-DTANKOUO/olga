# Examples

Five runnable scripts that live in
[`olgadoc/examples/`](https://github.com/Hugues-DTANKOUO/olga/tree/main/olgadoc/examples)
and cover the most common pipelines.

| Example | Demonstrates |
| --- | --- |
| [Quickstart](./quickstart.md) | `Document.open`, per-page preview |
| [Extract tables](./extract_tables.md) | `doc.tables()`, cross-page tables, the `Table` payload |
| [Batch processability](./batch_processability.md) | `doc.processability()`, health gating a directory |
| [Search and extract](./search_and_extract.md) | `doc.search()`, `SearchHit` fields |
| [JSON walk](./json_walk.md) | `doc.to_json()`, discriminated `JsonElement` narrowing |

Every script accepts `--help` via argparse or prints its usage when run
without arguments.
