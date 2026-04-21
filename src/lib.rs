// SPDX-License-Identifier: Apache-2.0

//! # Olga
//!
//! Intelligent Document Processing engine — structure and content extraction
//! from multi-format documents (PDF, DOCX, XLSX, HTML) with provenance tracking.
//!
//! ## Architecture
//!
//! The pipeline processes documents as streams of structural elements:
//!
//! ```text
//! [Format Decoder] → Stream<Primitive> → [Page Buffer] → [Reading Order] → [Detectors] → [Assembler] → Stream<DocumentNode>
//! ```
//!
//! Three output paths are supported:
//! - **JSON** — full pipeline: decode → structure engine → tree rendering
//! - **Text** — spatial renderer: character-level geometry from pdf_oxide, bypasses structure engine
//! - **Markdown** — spatial + inline formatting (bold, italic, links), bypasses structure engine
//!
//! ## Core types
//!
//! - [`model::Primitive`] — the atomic visual element (text, line, rectangle, image)
//! - [`model::SemanticHint`] — optional structural annotation on a primitive
//! - [`model::DocumentNode`] — a node in the output structure tree
//! - [`model::BoundingBox`] — normalized (0.0–1.0) position on a page
//! - [`model::PageDimensions`] — page size, rotation, and coordinate normalization

pub mod api;
pub mod error;
pub mod formats;
pub mod model;
pub mod output;
pub(crate) mod pipeline; // PrimitiveSink trait for incremental decoder emission
pub mod structure;
pub mod traits;
