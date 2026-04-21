# olgadoc — runnable examples

Five self-contained scripts that demonstrate the public API. Every
example accepts paths on the command line and prints to stdout, so they
double as smoke tests when you've just installed the wheel.

| Script                    | Demonstrates                                                |
| ------------------------- | ----------------------------------------------------------- |
| `quickstart.py`           | `Document.open`, per-page text preview                      |
| `extract_tables.py`       | `Document.tables()`, the `Table` payload, cross-page tables |
| `batch_processability.py` | `Document.processability()`, batch health gating            |
| `search_and_extract.py`   | `Document.search()`, `SearchHit` fields                     |
| `json_walk.py`            | `Document.to_json()`, the typed `JsonElement` union         |

## Running

From the `olgadoc/` directory, after `maturin develop` (or `pip install
olgadoc`):

```bash
python examples/quickstart.py ../tests/corpus/pdf/structured_report.pdf
python examples/extract_tables.py ../tests/corpus/xlsx/employee_directory.xlsx
python examples/batch_processability.py ../tests/corpus/
python examples/search_and_extract.py ../tests/corpus/pdf/structured_report.pdf "report"
python examples/json_walk.py ../tests/corpus/html/complex_report.html --depth 2
```

Every script ships with a docstring at the top — `python examples/<name>.py`
without arguments prints the usage.
