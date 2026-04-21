Minimal corpus fixtures for library validation.

Layout:
- `html/`: real HTML input consumed directly by the decoder
- `docx/`: source-like OOXML body snippets plus minimal DOCX archives
- `pdf/`: source-like text payloads plus minimal valid PDFs
- `xlsx/`: source-like TSV grids plus minimal XLSX archives

Fixtures are committed to the repository (total ~140 KB) so the test suite is
hermetic — `cargo test` and `cargo llvm-cov` must succeed on a clean checkout
without any download step. Keep new fixtures small and source-like where
possible; the binary archives exist to exercise the end-to-end decoder path
that a pure source-level fixture cannot cover.
