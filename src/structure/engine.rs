//! StructureEngine — the public API that wires the full pipeline.
//!
//! Orchestrates the four internal stages into a single `structure()` call:
//!
//! ```text
//! DecodeResult.primitives
//!   → PageBuffer::ingest()           [page by page, O(1 page) memory]
//!     → ReadingOrderResolver         [per page — Phase 2]
//!       → StructureDetector::detect  [per page, unhinted only — Phase 4]
//!         → Assembler::process_page  [cross-page FSM — Phase 3]
//!           → StructureResult        [DocumentNode tree]
//! ```
//!
//! # Usage
//!
//! ```
//! use olga::structure::{StructureConfig, StructureEngine};
//! use olga::traits::DecodeResult;
//! use olga::model::{DocumentFormat, DocumentMetadata, PaginationBasis, PaginationProvenance};
//!
//! let engine = StructureEngine::new(StructureConfig::default())
//!     .with_default_detectors();
//!
//! let decode_result = DecodeResult {
//!     metadata: DocumentMetadata {
//!         format: DocumentFormat::Pdf,
//!         page_count: 0,
//!         page_count_provenance: PaginationProvenance::synthetic(PaginationBasis::FallbackDefault),
//!         page_dimensions: vec![],
//!         encrypted: false,
//!         text_extraction_allowed: true,
//!         file_size: 0,
//!         title: None,
//!     },
//!     primitives: vec![],
//!     warnings: vec![],
//!     images: vec![],
//! };
//!
//! let result = engine.structure(decode_result);
//! assert!(result.root.children.is_empty());
//! ```

mod builder;
mod helpers;
mod process;

use crate::error::Warning;
use crate::model::{DocumentMetadata, DocumentNode};

use super::detectors::StructureDetector;
use super::layout::LayoutAnalyzer;
use super::types::StructureConfig;

/// The public entry point for document structuring.
///
/// Owns a [`StructureConfig`] and a set of pluggable [`StructureDetector`]s.
/// Call [`structure`](Self::structure) to process a [`DecodeResult`] into a
/// [`StructureResult`].
///
/// # Pipeline
///
/// Internally, the engine runs five stages per page:
///
/// 1. **Page Buffer** — accumulates primitives, flushes at page boundaries.
/// 2. **Reading Order** — selects and applies the best per-page reorder strategy.
/// 3. **Layout Analysis** — groups text primitives into `TextBox`es (lines,
///    columns, blocks). Results are passed to detectors via `DetectionContext`.
/// 4. **Heuristic Detectors** — enrich unhinted pages with structural hints,
///    optionally using `TextBox`es for more robust analysis.
/// 5. **Assembler** — hint-driven FSM producing the `DocumentNode` tree.
///
/// The buffer and assembler maintain cross-page state (font histogram,
/// section stack). Detectors and reading order are stateless per-page
/// operations.
pub struct StructureEngine {
    pub(super) config: StructureConfig,
    pub(super) detectors: Vec<Box<dyn StructureDetector>>,
    /// Optional layout analyzer for geometric text block analysis.
    /// When set, runs on unhinted pages before detectors to produce
    /// `TextBox`es that inform list/table detection.
    pub(super) layout_analyzer: Option<Box<dyn LayoutAnalyzer>>,
}

/// The output of the structuring pipeline.
///
/// Contains the hierarchical document tree, page artifacts (headers/footers),
/// all warnings accumulated across decoding and structuring, and a list of
/// pages where heuristic detection was applied.
#[derive(Debug)]
pub struct StructureResult {
    /// Document metadata from the decode phase.
    pub metadata: DocumentMetadata,
    /// Root `Document` node containing the entire tree.
    pub root: DocumentNode,
    /// Page artifacts (headers/footers) stripped from the content tree.
    ///
    /// Only populated when `StructureConfig::strip_artifacts` is `true`.
    pub artifacts: Vec<DocumentNode>,
    /// All warnings from both decoding and structuring phases.
    pub warnings: Vec<Warning>,
    /// Pages where heuristic detection was applied (hint coverage was below
    /// the trust threshold).
    ///
    /// Empty for fully hinted documents (DOCX, HTML, tagged PDF).
    pub heuristic_pages: Vec<u32>,
}

#[cfg(test)]
#[path = "engine/tests.rs"]
mod tests;
