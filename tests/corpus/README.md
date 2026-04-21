Minimal corpus fixtures for library validation.

These fixtures are intentionally source-like and human-readable:
- `html/`: real HTML input consumed directly by the decoder
- `docx/`: OOXML body snippets wrapped into a minimal DOCX archive by tests
- `pdf/`: text payloads wrapped into a minimal valid PDF by tests
- `xlsx/`: TSV grid data wrapped into a minimal XLSX archive by tests

The goal is stable, versioned coverage of decoder contracts without introducing
binary fixtures into the repository.
