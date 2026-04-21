//! Structure Engine — transforms raw Primitives into a structured DocumentNode tree.
//!
//! This module is the brain of the Olga pipeline. It consumes the
//! `DecodeResult` from any format decoder and produces a hierarchical
//! document tree with provenance tracking.
//!
//! # Architecture
//!
//! ```text
//! DecodeResult.primitives
//!   → PageBuffer::ingest()      [page by page, O(1 page) memory]
//!     → ReadingOrderResolver    [per page — Phase 2]
//!       → StructureDetector     [per page, unhinted only — Phase 4]
//!         → Assembler           [cross-page FSM — Phase 3]
//!           → StructureResult   [DocumentNode tree]
//! ```
//!
//! # Current status
//!
//! Phases 1 (Page Buffer), 2 (Reading Order Resolver), 3 (Assembler),
//! 4 (Heuristic Detectors), and 5 (StructureEngine public API) are
//! implemented. Cross-page continuity is implemented for the current
//! v1 scope: tables and lists survive page boundaries via lookahead,
//! repeated table headers are skipped, and paragraphs can merge across
//! pages using continuation-text and geometry-aware checks. The assembler
//! uses a tiered priority system in `classify()` so structural containers
//! (TableCell, ListItem) always outrank content types (Paragraph, Heading).

pub mod assembler;
pub mod buffer;
pub mod detectors;
pub mod engine;
pub mod layout;
pub mod reading_order;
pub mod types;
pub mod visual_lines;

// Re-export the most commonly used types.
pub use assembler::{Assembler, AssemblyResult};
pub use buffer::PageBuffer;
pub use detectors::{
    DetectedHint, DetectionContext, HeadingDetector, ListDetector, ParagraphDetector,
    StructureDetector, TableDetector, apply_detectors,
};
pub use engine::{StructureEngine, StructureResult};
pub use layout::{HeuristicLayoutAnalyzer, LAParams, LayoutAnalyzer, TextBox, TextLine, TextSpan};
pub use reading_order::{
    ColumnAwareResolver, ReadingOrderResolver, SimpleResolver, TaggedOrderResolver, select_resolver,
};
pub use types::{FontHistogram, PageFlush, StructureConfig};
pub use visual_lines::{
    AlignedSpan, PageMetrics, VisualLine, group_visual_lines, looks_like_table, render_line,
};
