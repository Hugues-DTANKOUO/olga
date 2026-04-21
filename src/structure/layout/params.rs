//! Layout analysis parameters for the heuristic layout analyzer.
//!
//! The parameter model (names, default ranges, Y-overlap formula) is
//! informed by pdfminer.six's `LAParams` (MIT) — see ATTRIBUTION.md.
//! Defaults are tuned on Olga's corpus, not inherited.

/// Configuration for the heuristic layout analyzer.
///
/// These parameters control the three stages of hierarchical grouping.
/// Default values are calibrated for typical business/academic documents.
#[derive(Debug, Clone)]
pub struct LAParams {
    // -- Stage 1: Characters → Lines --
    /// Maximum vertical distance (relative to character height) for two
    /// characters to be considered on the same line.
    ///
    /// Two characters are on the same line if the vertical overlap of their
    /// bounding boxes exceeds `1.0 - line_margin` of the smaller character's
    /// height.
    ///
    /// Default: 0.5. Higher values produce more permissive line grouping.
    pub line_margin: f32,

    /// Gap multiplier for word boundary detection within a line.
    ///
    /// Follows the classical layout-analysis convention: a gap between
    /// two characters larger than `word_margin × char_width` inserts a
    /// word boundary.
    ///
    /// **Currently unused.** Our decoders emit pre-grouped `Text` spans
    /// (words or phrases), not individual characters, so intra-line word
    /// boundary detection is not needed. This parameter is preserved for
    /// future compatibility if a char-level PDF decoder is added.
    ///
    /// Default: 0.1 (10% of average char width).
    pub word_margin: f32,

    // -- Stage 2: Lines → Text Boxes --
    /// Controls how aggressively lines are grouped into boxes and how boxes
    /// are ordered.
    ///
    /// - `None` — order purely by geometric closeness (area metric).
    /// - `Some(0.5)` (default) — hybrid: favor top-to-bottom then left-to-right,
    ///   but allow proximity to override when lines are close.
    /// - `Some(1.0)` — strict top-to-bottom ordering.
    /// - `Some(-1.0)` — pure proximity ordering.
    pub boxes_flow: Option<f32>,

    /// Maximum vertical gap between lines (relative to average line height)
    /// to group them into the same text box.
    ///
    /// Default: 0.5. Higher values produce larger boxes (more aggressive merging).
    pub line_overlap_threshold: f32,

    // -- Stage 3: Reading order --
    /// Minimum horizontal gap (as fraction of page width) to detect a column
    /// boundary between text boxes.
    ///
    /// Default: 0.05 (5% of page width).
    pub column_gap_fraction: f32,
}

impl Default for LAParams {
    fn default() -> Self {
        Self {
            line_margin: 0.5,
            word_margin: 0.1,
            boxes_flow: Some(0.5),
            line_overlap_threshold: 0.5,
            column_gap_fraction: 0.05,
        }
    }
}
