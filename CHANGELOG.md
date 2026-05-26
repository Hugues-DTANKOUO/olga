# Changelog

All notable changes to this project are documented here. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
this project adheres to [Semantic Versioning](https://semver.org/).

Starting with v0.2.0, each release will be documented with granular
`Added` / `Changed` / `Fixed` / `Removed` / `Security` sections. v0.1.0
is the single foundational cut that establishes the baseline.

## [0.1.3] — 2026-05-26

> Fix : preserve merge span on header cells (`HintKind::TableHeader` now carries `rowspan` + `colspan`).

### Fixed

- **Merged header cells now surface their geometric span faithfully.** Prior to v0.1.3, `HintKind::TableHeader` only carried `col: u32` ; the merge span was computed internally by the XLSX decoder (`SheetContext::merge_span`) and the non-origin cells were correctly skipped (`is_merged_non_origin`), but the public `api::TableCell.rowspan` / `colspan` for header cells were hardcoded to `1` regardless of the actual merge geometry. Downstream consumers walking `Document::tables()` saw `(0,0) "Identité" colspan=1`, `(0,2) "Tarification" colspan=1`, `(0,4) "Stock" colspan=1` for a row-0 header band that merged `(0,0)..(0,1)` + `(0,2)..(0,3)` — the column gaps signaled the merges but the geometric span was lost at the API frontier. This made it harder to write robust consumers for XLSX with category-level header groupings (a common real-world pattern).

### Changed

- **`HintKind::TableHeader`** extended from `{ col: u32 }` to `{ col: u32, rowspan: u32, colspan: u32 }`. The two new fields default to `1` for the non-merged case and reflect the source document's geometric merge span for merged origins. **Source-incompatible for consumers destructuring all fields** (existing consumers using `matches!(.., HintKind::TableHeader { col: N, .. })` keep working ; consumers using `{ col }` need to update to `{ col, .. }` or `{ col, rowspan, colspan }`).
- All 5 emit sites updated to thread the rowspan/colspan through :
  - `formats/xlsx/sheets/emit.rs` (heuristic header path + `native_table_hint`)
  - `formats/pdf/tagged/semantics.rs` (passes `cell.rowspan` / `cell.colspan` from active cell)
  - `formats/html/walker/structures.rs` (passes the parsed `<th rowspan=… colspan=…>` attributes)
  - `formats/docx/body/tables.rs` (passes `cell.rowspan` / `cell.colspan` from active cell)
  - `structure/detectors/table/lattice/detect.rs` (defaults to `1` / `1` — lattice-detected tables don't carry merge geometry from heuristics)
- `structure/assembler/classify.rs` propagates the new fields into `PrimaryRole::TableCell { rowspan, colspan, is_header: true, ... }` instead of hardcoding `1` / `1`.
- `structure/detectors/table/continuation.rs::promote_to_continuation` preserves the span when promoting `TableHeader` → `TableCellContinuation` (instead of dropping to `1` / `1`).

### Downstream impact

Aliya's `crates/olga-bridge` (Chunk 3.1, ADR-0068 D9.6) discovered this gap empirically while exercising the messy-real-world resilience fixture : the bridge re-export surface `olga_bridge::TableCell` showed `max_colspan = 1` even when the input xlsx had `merge_cells("Identité", colspan=2)`. The fix unblocks the consumer-side T pipeline at Chunk 5.1 onboarding — instead of inferring merges by gap detection, the T-pipeline dispatches directly on `TableCell.colspan > 1`.

## [0.1.2] — 2026-05-25

> Per-format feature flags — opt-out binary size for downstream consumers (e.g. mobile surfaces).

### Added

- **`[features]` block** in `Cargo.toml` exposing 4 per-format feature flags : `xlsx` / `pdf` / `docx` / `html`. Default feature set is `["xlsx", "pdf", "docx", "html"]` — full backward compatibility with v0.1.1 behavior (no change for consumers who don't opt out).
- A private umbrella `_ooxml` feature that pulls in the shared `quick-xml` + `zip` deps that both DOCX and XLSX need ; gated transitively by `xlsx` and `docx` features. Consumers should NOT enable `_ooxml` directly.
- Compile-time feature-disabled error path : when a format's feature is opted out, `Document::open()` and the CLI `select_decoder()` dispatch return `IdpError::UnsupportedFormat` carrying a "feature disabled at compile time" message. The `Format` enum keeps all 4 variants visible so the public API stays stable across feature combinations.

### Changed

- Format-specific dependencies are now `optional = true` in `Cargo.toml` :
  - `calamine` → gated by `xlsx`
  - `pdf_oxide` → gated by `pdf`
  - `scraper` + `ego-tree` → gated by `html`
  - `quick-xml` + `zip` → gated by `_ooxml` (transitively by `xlsx` and `docx`)
- The PDF spatial renderer modules (`output::markdown` + `output::spatial` + `output::rules` + helpers `col_resolve` / `row_cluster`) are now cfg-gated on the `pdf` feature. These modules consume `pdf_oxide::layout::TextChar` directly and have no meaningful rendering path without PDF support.
- Integration test files (`tests/docx_tests.rs` / `tests/pdf_tests.rs` / `tests/xlsx_tests.rs` / `tests/html_tests.rs`) gated with `#![cfg(feature = "X")]` — disabled formats compile to an empty test-binary shell with zero tests.

### Compatibility

Fully backward-compatible with v0.1.1 for consumers using `olga = "0.1"` without explicit feature manipulation : the default feature set replicates v0.1.1's all-formats behavior verbatim. Consumers opting out of formats (e.g. `default-features = false, features = ["xlsx", "html"]`) gain measurable binary-size reduction.

### Downstream impact

Aliya's `crates/olga-bridge` (Chunk 3.1, ADR-0067) consumes this release to support its mobile-surface feature-flag discipline. The bridge propagates the 4 feature flags one-to-one so Aliya mobile builds can opt out of PDF + DOCX while keeping XLSX + HTML for the dominant onboarding flow (Excel catalog import + product-page web scraping).

## [0.1.1] — 2026-04-21

> Docs patch — surface the independent v0.1.0 benchmark on PyPI and crates.io.

No engine changes. This release publishes the independent,
reproducible post-release audit at
[`olga_v0.1.0_benchmark/`](https://github.com/Hugues-DTANKOUO/olga/tree/main/olga_v0.1.0_benchmark)
and links it from the crate README, the PyPI README, the MkDocs
landing page, and `BENCHMARKS.md`. Headline result on a 50-file
mixed-format corpus: 1.62× faster and 2.62× more extracted content
than a hand-routed best-of-breed pipeline. The crate metadata author
field is also corrected from "Hugues Tankouo" to "Hugues Dtankouo".

## [0.1.0] — 2026-04-21

> First public release — the end-to-end Olga pipeline in one cut.

Olga's first public release. This foundational cut ships the full
intelligent document processing pipeline: a Rust core that parses PDF,
DOCX, XLSX, and HTML with provenance tracking and table
reconstruction, a Python distribution (`olgadoc`) with a strictly-typed
API surface, an `olga` CLI for inspection, extraction, search, and
page-level access, runnable examples, end-to-end regression coverage,
an MkDocs site, and a full CI/CD pipeline publishing to crates.io and
PyPI. The public API is stable enough for evaluation and prototyping;
expect minor breaking changes on the path to 1.0.

[Unreleased]: https://github.com/Hugues-DTANKOUO/olga/compare/v0.1.2...HEAD
[0.1.2]: https://github.com/Hugues-DTANKOUO/olga/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/Hugues-DTANKOUO/olga/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/Hugues-DTANKOUO/olga/releases/tag/v0.1.0
