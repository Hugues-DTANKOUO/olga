# Changelog

All notable changes to this project are documented here. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
this project adheres to [Semantic Versioning](https://semver.org/).

Starting with v0.2.0, each release will be documented with granular
`Added` / `Changed` / `Fixed` / `Removed` / `Security` sections. v0.1.0
is the single foundational cut that establishes the baseline.

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

[Unreleased]: https://github.com/Hugues-DTANKOUO/olga/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Hugues-DTANKOUO/olga/releases/tag/v0.1.0
