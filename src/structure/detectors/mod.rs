//! Heuristic structure detectors for untagged documents.
//!
//! These detectors run on pages that lack format-derived hints (e.g.,
//! untagged PDF). They analyze primitive geometry and typography to
//! infer structural roles, emitting [`SemanticHint`]s that the Assembler
//! can consume.
//!
//! # Design
//!
//! Detectors are pluggable via the [`StructureDetector`] trait. They run
//! **after** reading-order correction, **before** the Assembler. Each
//! detector independently analyzes the page and produces hints for
//! specific primitive indices.
//!
//! # Shipped detectors (Phase 4)
//!
//! - [`ParagraphDetector`]: Groups consecutive text primitives into
//!   paragraphs based on Y-spacing analysis.
//! - [`HeadingDetector`]: Identifies headings from font-size outliers
//!   using the cumulative font histogram.
//!
//! # Ordering
//!
//! When multiple detectors produce hints for the same primitive, the
//! hint with the highest confidence wins. Existing format-derived hints
//! are never overridden — detectors skip primitives that already carry
//! hints.

pub mod heading;
pub mod list;
pub mod paragraph;
pub mod table;

pub use heading::HeadingDetector;
pub use list::ListDetector;
pub use paragraph::ParagraphDetector;
pub use table::TableDetector;

use crate::model::{PageDimensions, Primitive, SemanticHint};
use crate::structure::layout::TextBox;
use crate::structure::types::FontHistogram;

// ---------------------------------------------------------------------------
// StructureDetector trait
// ---------------------------------------------------------------------------

/// A pluggable heuristic that infers structural roles from primitive geometry.
///
/// Detectors analyze a single page of primitives (already in reading order)
/// and produce hints to attach to specific primitives by index. They MUST
/// NOT modify the primitives slice — they only produce new hints.
///
/// Implementations must be `Send + Sync` to support future parallel-page
/// processing.
pub trait StructureDetector: Send + Sync {
    /// Human-readable name for debugging and provenance tracking.
    fn name(&self) -> &str;

    /// Analyze a page of primitives and produce structural hints.
    ///
    /// Returns a list of `(primitive_index, hint)` pairs. The caller
    /// is responsible for attaching hints to the corresponding primitives.
    ///
    /// **Contract:** Detectors MUST skip primitives that already have
    /// format-derived hints (check `primitive.has_format_hints()`).
    fn detect(&self, page: &[Primitive], context: &DetectionContext) -> Vec<DetectedHint>;
}

/// A hint produced by a detector, targeted at a specific primitive.
#[derive(Debug, Clone)]
pub struct DetectedHint {
    /// Index of the primitive in the page slice.
    pub primitive_index: usize,
    /// The hint to attach.
    pub hint: SemanticHint,
}

/// Context available to detectors during analysis.
///
/// Carries cross-page state (font histogram), per-page metadata
/// (page number, dimensions), optional layout analysis results
/// (`text_boxes`), and cross-page continuation context for informed
/// heuristic decisions.
#[derive(Debug)]
pub struct DetectionContext {
    /// Cumulative font-size histogram across all pages seen so far.
    /// Used by the heading detector to identify body vs. heading sizes.
    pub font_histogram: FontHistogram,
    /// Current page number (0-indexed).
    pub page: u32,
    /// Page dimensions (when available). PDF provides these natively;
    /// DOCX/XLSX/HTML may not.
    pub page_dims: Option<PageDimensions>,
    /// Text boxes produced by the layout analyzer, if one was configured.
    ///
    /// When `Some`, detectors can use pre-grouped text blocks for more
    /// robust analysis (e.g., measuring indentation at the textbox level
    /// instead of per-primitive, or detecting column alignment from box
    /// centroids). When `None`, detectors fall back to raw primitive
    /// analysis.
    pub text_boxes: Option<Vec<TextBox>>,
    /// Column X-positions from a table that is currently open in the
    /// assembler (carried across page boundaries).
    ///
    /// When `Some`, the [`TableDetector`] can compare its detected columns
    /// against these positions to determine whether a table on this page
    /// is a continuation of the previous page's table rather than a new
    /// independent table. Positions are sorted ascending in normalized
    /// page coordinates (0.0–1.0).
    pub open_table_columns: Option<Vec<f32>>,
}

// ---------------------------------------------------------------------------
// Detector registry helper
// ---------------------------------------------------------------------------

/// Apply all detectors to a page of primitives, attaching the highest-
/// confidence hint per primitive.
///
/// Primitives that already carry hints are skipped. When multiple
/// detectors produce hints for the same primitive, the one with the
/// highest confidence wins.
pub fn apply_detectors(
    primitives: &mut [Primitive],
    detectors: &[Box<dyn StructureDetector>],
    context: &DetectionContext,
) {
    if detectors.is_empty() {
        return;
    }

    // Collect all detected hints, keyed by primitive index.
    // For each index, keep only the highest-confidence hint.
    let mut best_hints: Vec<Option<SemanticHint>> = vec![None; primitives.len()];

    for detector in detectors {
        let hints = detector.detect(primitives, context);
        for dh in hints {
            if dh.primitive_index >= primitives.len() {
                continue; // Guard against out-of-bounds.
            }
            // Skip primitives that already have format-derived hints.
            if primitives[dh.primitive_index].has_format_hints() {
                continue;
            }
            let slot = &mut best_hints[dh.primitive_index];
            match slot {
                Some(existing) if existing.confidence >= dh.hint.confidence => {
                    // Keep existing higher-confidence hint.
                }
                _ => {
                    *slot = Some(dh.hint);
                }
            }
        }
    }

    // Attach winning hints to primitives.
    for (i, hint_opt) in best_hints.into_iter().enumerate() {
        if let Some(hint) = hint_opt {
            primitives[i].hints.push(hint);
        }
    }
}
