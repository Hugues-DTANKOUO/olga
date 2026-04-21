# Attribution and License Notices

This file documents the third-party code, algorithms, and libraries
used in Olga, along with their licenses. It MUST be included in all
distributed binaries and source releases per section 4 of the Apache
License, Version 2.0 (see [LICENSE](./LICENSE)).

The list below reflects the direct dependencies declared in the
workspace manifests (`Cargo.toml` and `olgadoc/Cargo.toml`) at the time
of the current release. Transitive dependencies are covered by their
respective licenses inside the published wheel and crate tarballs; a
full machine-generated inventory can be produced with
`cargo about generate` or `cargo deny list` before each release.

---

## Direct dependencies — engine crate (`olga`)

### pdf_oxide
- **License:** Apache-2.0 (verify at publish time)
- **Source:** https://crates.io/crates/pdf_oxide
- **Usage:** PDF parsing and low-level object access

### quick-xml
- **License:** MIT
- **Copyright:** quick-xml contributors
- **Source:** https://github.com/tafia/quick-xml
- **Usage:** XML parsing for DOCX / OOXML processing

### calamine
- **License:** MIT
- **Copyright:** calamine contributors
- **Source:** https://github.com/tafia/calamine
- **Usage:** XLSX spreadsheet reading

### scraper
- **License:** ISC
- **Copyright:** scraper contributors
- **Source:** https://github.com/causal-agent/scraper
- **Usage:** HTML parsing and CSS selector queries

### ego-tree
- **License:** ISC
- **Copyright:** ego-tree contributors
- **Source:** https://github.com/causal-agent/ego-tree
- **Usage:** Tree data structure used by the HTML and structural
  pipelines

### zip
- **License:** MIT
- **Source:** https://github.com/zip-rs/zip2
- **Usage:** OOXML (DOCX / XLSX) container reading

### serde + serde_json
- **License:** MIT OR Apache-2.0
- **Copyright:** David Tolnay and serde contributors
- **Source:** https://github.com/serde-rs/serde,
  https://github.com/serde-rs/json
- **Usage:** Serialization / deserialization of structured document
  payloads

### rstar
- **License:** MIT OR Apache-2.0
- **Source:** https://github.com/georust/rstar
- **Usage:** R-tree spatial indexing for layout analysis (replaces the
  quadratic search used by comparable Python tools)

### unicode-normalization
- **License:** MIT OR Apache-2.0
- **Source:** https://github.com/unicode-rs/unicode-normalization
- **Usage:** Unicode NFC normalization of extracted text

### thiserror
- **License:** MIT OR Apache-2.0
- **Copyright:** David Tolnay
- **Source:** https://github.com/dtolnay/thiserror
- **Usage:** Error enum derivation for the crate's error types

### log
- **License:** MIT OR Apache-2.0
- **Source:** https://github.com/rust-lang/log
- **Usage:** Logging facade

### clap
- **License:** MIT OR Apache-2.0
- **Source:** https://github.com/clap-rs/clap
- **Usage:** CLI argument parsing for the `olga` binary

---

## Direct dependencies — Python bindings crate (`olgadoc`)

### pyo3
- **License:** MIT OR Apache-2.0
- **Source:** https://github.com/PyO3/pyo3
- **Usage:** Rust ↔ CPython bindings (abi3-py38, extension-module)

### serde_json
- **License:** MIT OR Apache-2.0
- **Copyright:** David Tolnay
- **Source:** https://github.com/serde-rs/json
- **Usage:** JSON bridging between the Rust core and the Python layer

---

## Reference implementations and prior art

Olga's untagged-PDF pipeline draws on several well-established projects
for vocabulary, parameter conventions, and algorithmic inspiration. The
Rust implementation is an independent reimplementation, not a line-by-line
port: it diverges in data structures, algorithmic complexity, module
boundaries, and cross-page handling. The projects listed below are
credited for their role as the clearest prior art in the field.

Scope note: the pdfminer.six and pdfplumber references apply only to
the **untagged-PDF** code path (heuristic layout analysis and
borderless / lattice table detection). DOCX, XLSX, HTML, and Tagged PDF
use the source format's own structural markup and do not go through
this pipeline. The Apache POI reference applies to the **XLSX
number-format** code path (`src/formats/xlsx/number_format/`) and to
the **DOCX secondary-story ordering** convention in
`src/formats/docx/decode/package.rs` / `src/output/prim_spatial/blocks.rs`
(headers → body → footers → notes → comments — the ordering also used
by python-docx and Mammoth). The SheetJS SSF reference applies to the
XLSX number-format test corpus only.

### pdfminer.six — heuristic layout analysis
- **License:** MIT
- **Copyright:** Yusuke Shinyama and pdfminer.six contributors
- **Source:** https://github.com/pdfminer/pdfminer.six
- **Relation to Olga:** The hierarchical `chars → lines → boxes` shape
  and the `LAParams` family of tolerances (`line_margin`, `char_margin`,
  `word_margin`, `boxes_flow`) in Olga's `HeuristicLayoutAnalyzer` are
  informed by pdfminer.six's `layout.py`. The pairwise (non-expansive)
  line-clustering convention is likewise aligned with pdfminer.six.
- **Olga's divergences:** Rust reimplementation with streaming
  primitives; R-tree (rstar) spatial indexing replacing quadratic
  scans; an X-gap heuristic for column / gutter detection; different
  module boundaries and type surface; integration with Olga's semantic
  hint model and structure engine.

### pdfplumber — table detection
- **License:** MIT
- **Copyright:** Jeremy Singer-Vine and pdfplumber contributors
- **Source:** https://github.com/jsvine/pdfplumber
- **Relation to Olga:** The 5-step shape of Olga's untagged-PDF
  `TableDetector` pipeline (edge extraction → snap/merge → intersections
  → cells → tables), the parameter names (`snap_tolerance`,
  `join_tolerance`, `intersection_tolerance`, `min_edge_length`), the
  three-pass x-alignment clustering for stream tables, and the
  four-corner `_cells_from_intersections` approach are informed by
  pdfplumber's `table.py` and `utils/geometry.py`.
- **Olga's divergences:** Rust reimplementation; virtual-edge
  generation for partial grids; cross-page table continuation;
  header-row detection and rowspan / colspan inference; spatial-hash
  intersection deduplication (O(n) instead of O(n²)); binary-search
  narrowing of intersection candidates; a configurable
  `CellValidation` (Strict / Relaxed) dimension that pdfplumber does
  not expose.

### Apache POI — Excel number-format grammar
- **License:** Apache-2.0
- **Copyright:** The Apache Software Foundation
- **Source:** https://poi.apache.org/
- **Relation to Olga:** The section-splitting and
  `CellFormatPart`-style parsing strategy in
  `src/formats/xlsx/number_format/` is informed by POI, which is the
  most legible reference implementation of Excel's four-section
  format grammar (positive / negative / zero / text) and associated
  sign-handling rules (`hasSign`, the ambiguous-`m` month / minute
  resolver, the Lotus 1900 leap-year bug, fractional-second rounding).
  The rendering code itself (`number_formatter.rs`, `date_formatter.rs`,
  `general_formatter.rs`, `text_formatter.rs`, `section.rs`) is an
  independent Rust implementation aligned on POI's observable output
  contract rather than transposed from POI's Java source — different
  control flow, different data structures, no copied class hierarchy.
- **Test fixtures (Apache-2.0 derivation):** The assertion tables in
  `src/formats/xlsx/number_format/poi_fixtures.rs` (guarded by
  `#[cfg(test)]`) are direct transliterations of
  `(format, value, expected)` triples drawn from POI's
  `TestDataFormatter` (`org.apache.poi.ss.format`). They are test-only
  derivative data. Olga is distributed under Apache-2.0, which matches
  POI's own license, and this attribution entry is the §4(c) notice
  required for source-form redistribution of derivative work. The
  SheetJS SSF fixtures in `ssf_fixtures.rs` are analogous
  transliterations of `test.tsv` rows from the SheetJS SSF corpus;
  see the SheetJS entry below for the upstream license.

### SheetJS SSF — Excel number-format cross-engine validation
- **License:** Apache-2.0
- **Copyright:** SheetJS LLC and SSF contributors
- **Source:** https://github.com/SheetJS/ssf
- **Usage:** The fixtures in
  `src/formats/xlsx/number_format/ssf_fixtures.rs` (`#[cfg(test)]`)
  are transliterations of assertions from SSF's public `test/`
  corpus, used as the second oracle alongside POI for Excel
  number-format rendering. No SSF runtime code is linked into Olga.

---

## Contributor License Agreement (CLA)

Olga is distributed under the
[Apache License, Version 2.0](./LICENSE). Apache-2.0 is a permissive
license that includes an explicit patent grant; per section 5 of the
License, contributions intentionally submitted for inclusion are
automatically licensed under those same terms unless the contributor
explicitly states otherwise. No separate CLA template is required
(see [CONTRIBUTING.md](./CONTRIBUTING.md)).
