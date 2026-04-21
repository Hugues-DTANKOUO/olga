//! Layout analysis module — groups characters into lines and text boxes.
//!
//! This module produces logical text blocks from raw character-level
//! primitives. It is the foundation for structure detection on **untagged**
//! PDFs only; tagged PDFs use their embedded structure tree
//! (`src/formats/pdf/tagged/`) and bypass this pipeline entirely.
//!
//! # Three-stage pipeline
//!
//! ```text
//! Characters (Primitives with PrimitiveKind::Text)
//!   → Stage 1: group into TextLines (Y-overlap + X-sort)
//!     → Stage 2: group into TextBoxes (R-tree spatial proximity)
//!       → Stage 3: determine reading order + column detection
//! ```
//!
//! # Integration
//!
//! The [`LayoutAnalyzer`] trait produces [`TextBox`]es from a page of
//! primitives. The [`HeuristicLayoutAnalyzer`] is Olga's default
//! implementation. Results are used by the Structure Engine's detector
//! pipeline to emit `SemanticHint`s for untagged pages.
//!
//! # Prior art
//!
//! The three-stage shape and the `LAParams` parameter model take inspiration
//! from pdfminer.six's `layout.py` (MIT). Olga's implementation diverges in
//! several substantial ways: R-tree spatial indexing (`rstar`) in place of
//! quadratic scans, a greedy/R-tree dispatcher by input size, an explicit
//! X-gap column-separation heuristic not present upstream, and two
//! reading-order strategies.

mod lines;
mod params;
mod textbox;
mod types;

pub use params::LAParams;
pub use types::{TextBox, TextLine, TextSpan};

use crate::model::Primitive;

// ---------------------------------------------------------------------------
// LayoutAnalyzer trait
// ---------------------------------------------------------------------------

/// Trait for layout analysis algorithms.
///
/// Takes a page of primitives and produces logical text boxes (grouped lines).
/// Implementations must be `Send + Sync` for future parallel processing.
pub trait LayoutAnalyzer: Send + Sync {
    /// Human-readable name for debugging and provenance.
    fn name(&self) -> &str;

    /// Analyze a page of primitives and produce text boxes.
    ///
    /// The primitives slice contains all primitives for a single page.
    /// Returns text boxes in reading order.
    fn analyze(&self, primitives: &[Primitive]) -> Vec<TextBox>;
}

// ---------------------------------------------------------------------------
// HeuristicLayoutAnalyzer
// ---------------------------------------------------------------------------

/// Default heuristic layout analyzer for untagged PDFs.
///
/// Three-stage hierarchical grouping:
/// 1. Characters → Lines (Y-overlap tolerance)
/// 2. Lines → Text Boxes (R-tree spatial proximity)
/// 3. Text Boxes ordered by reading order
///
/// The parameter model (`LAParams`) and the three-stage shape are informed
/// by pdfminer.six's `layout.py` (MIT); see [ATTRIBUTION.md](../../../../ATTRIBUTION.md)
/// for the full prior-art note.
#[derive(Default)]
pub struct HeuristicLayoutAnalyzer {
    pub params: LAParams,
}

impl HeuristicLayoutAnalyzer {
    pub fn new(params: LAParams) -> Self {
        Self { params }
    }
}

impl LayoutAnalyzer for HeuristicLayoutAnalyzer {
    fn name(&self) -> &str {
        "HeuristicLayoutAnalyzer"
    }

    fn analyze(&self, primitives: &[Primitive]) -> Vec<TextBox> {
        // Stage 1: extract text spans from primitives, group into lines.
        let spans = types::extract_text_spans(primitives);
        if spans.is_empty() {
            return Vec::new();
        }

        let text_lines = lines::group_spans_into_lines(&spans, &self.params);
        if text_lines.is_empty() {
            return Vec::new();
        }

        // Stage 2: group lines into text boxes using R-tree proximity.
        let text_boxes = textbox::group_lines_into_boxes(&text_lines, &self.params);

        // Stage 3: order text boxes by reading order.
        textbox::order_boxes(text_boxes, &self.params)
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
