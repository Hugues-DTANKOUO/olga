# Changelog

All notable changes to this project are documented here. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
this project adheres to [Semantic Versioning](https://semver.org/).

Starting with v0.2.0, each release will be documented with granular
`Added` / `Changed` / `Fixed` / `Removed` / `Security` sections. v0.1.0
is the single foundational cut that establishes the baseline.

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
