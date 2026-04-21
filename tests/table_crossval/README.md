# Table Detection Cross-Validation

Compares the IDP engine's table detection output against pdfplumber's
Python implementation on real PDF files.

> **Scope note (2026-04).** Per
> [ADR 0001](../../docs-dev/adr/0001-untagged-pdf-layout-preservation.md), the
> geometric `TableDetector` is **not invoked on untagged PDFs** in production.
> This cross-validation remains valid for measuring the detector's
> fidelity-to-pdfplumber on tagged PDFs and on arbitrary PDFs run through a
> direct detector invocation. Do **not** read a drop in this comparison as a
> regression to chase on untagged-PDF production output — production does not
> use this code path for untagged PDFs.

## Setup

1. Install pdfplumber: `pip install pdfplumber`

2. Generate reference data from a PDF:

   ```bash
   python tests/table_crossval/pdfplumber_extract.py \
       tests/corpus/pdf/structured_report.pdf \
       tests/table_crossval/structured_report_ref.json
   ```

3. Run the cross-validation test:

   ```bash
   cargo test --test table_crossval_tests
   ```

   The test exits early (passes) when the reference JSON is not found,
   so it is safe to run without pre-generated data. CI can generate the
   reference files as a pre-step to enable the full comparison.

## Match Criteria

- **Table count match**: same number of tables detected per page.
- **Grid size match**: rows and cols within +-1 of pdfplumber.
- **Cell text overlap**: >= 80% of pdfplumber's cell texts are also found
  in IDP engine output (Jaccard similarity on non-empty cell texts).

The 80% threshold accounts for differences in:
- Edge tolerance parameters (IDP uses normalized coordinates, pdfplumber
  uses PDF points).
- Text assignment heuristics (center-based vs containment-based).
- Span inference differences.
