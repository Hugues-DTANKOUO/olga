//! Paragraph detector — groups consecutive text primitives into paragraphs.
//!
//! # Algorithm
//!
//! 1. Scan text primitives in reading order (already sorted by Phase 2).
//! 2. Compute Y-gaps between consecutive text primitives.
//! 3. Compute the median line height and median Y-gap.
//! 4. Text primitives separated by Y-gap > `gap_factor` × median_gap are
//!    in different paragraphs. Those within the threshold are in the same
//!    paragraph.
//! 5. Non-text primitives break paragraph runs (they are structural
//!    separators like rules or images).
//!
//! # Limitations
//!
//! - Cannot detect paragraphs that start with an indent but have no
//!   vertical gap (e.g., novel text). Would require X-offset analysis.
//! - Does not handle multi-column layouts — column detection is in the
//!   Reading Order Resolver (Phase 2).
//! - `gap_factor` must be tuned per-corpus. Conservative default (1.5)
//!   avoids false paragraph splits on documents with generous line spacing.

use crate::model::{HintKind, Primitive, PrimitiveKind, SemanticHint};

use super::{DetectedHint, DetectionContext, StructureDetector};

// ---------------------------------------------------------------------------
// ParagraphDetector
// ---------------------------------------------------------------------------

/// Groups consecutive text primitives into paragraphs based on vertical
/// spacing analysis.
///
/// Confidence: 0.7 (geometric heuristic, not authoritative).
pub struct ParagraphDetector {
    /// Gap multiplier: a Y-gap larger than `gap_factor × median_gap`
    /// indicates a paragraph break. Default: 1.5.
    pub gap_factor: f32,
    /// Confidence assigned to detected paragraph hints. Default: 0.7.
    pub confidence: f32,
}

impl ParagraphDetector {
    /// Validate configuration values and clamp to sensible ranges.
    pub fn validate(&mut self) -> Vec<String> {
        let mut issues = Vec::new();

        if self.gap_factor <= 0.0 {
            issues.push(format!(
                "ParagraphDetector: gap_factor {} clamped to 1.5 (must be > 0.0)",
                self.gap_factor
            ));
            self.gap_factor = 1.5;
        }
        if self.confidence < 0.0 || self.confidence > 1.0 {
            issues.push(format!(
                "ParagraphDetector: confidence {} clamped to 0.7 (must be in [0.0, 1.0])",
                self.confidence
            ));
            self.confidence = 0.7;
        }

        issues
    }
}

impl Default for ParagraphDetector {
    fn default() -> Self {
        Self {
            gap_factor: 1.5,
            confidence: 0.7,
        }
    }
}

impl StructureDetector for ParagraphDetector {
    fn name(&self) -> &str {
        "paragraph"
    }

    fn detect(&self, page: &[Primitive], _context: &DetectionContext) -> Vec<DetectedHint> {
        // Collect indices of text primitives that don't already have hints.
        let text_indices: Vec<usize> = page
            .iter()
            .enumerate()
            .filter(|(_, p)| matches!(p.kind, PrimitiveKind::Text { .. }) && p.hints.is_empty())
            .map(|(i, _)| i)
            .collect();

        if text_indices.is_empty() {
            return Vec::new();
        }

        // Single text primitive → it's a paragraph (hint on first = only prim).
        if text_indices.len() == 1 {
            return vec![DetectedHint {
                primitive_index: text_indices[0],
                hint: SemanticHint::from_heuristic(
                    HintKind::Paragraph,
                    self.confidence,
                    "ParagraphDetector",
                ),
            }];
        }

        // Compute Y-gaps between consecutive text primitives.
        let gaps: Vec<f32> = text_indices
            .windows(2)
            .map(|pair| {
                let a = &page[pair[0]];
                let b = &page[pair[1]];
                // Gap = top of next primitive - bottom of current primitive.
                (b.bbox.y - (a.bbox.y + a.bbox.height)).max(0.0)
            })
            .collect();

        // Compute median gap.
        let median_gap = median(&gaps);

        // Threshold for paragraph breaks.
        let break_threshold = if median_gap > 0.0 {
            median_gap * self.gap_factor
        } else {
            // No gaps at all — all primitives are in one paragraph.
            f32::MAX
        };

        // Check for non-text primitives between consecutive text primitives
        // that would act as structural breaks.
        let has_non_text_between = |idx_a: usize, idx_b: usize| -> bool {
            // Check if any primitive between idx_a and idx_b (exclusive) is non-text.
            (idx_a + 1..idx_b).any(|i| !matches!(page[i].kind, PrimitiveKind::Text { .. }))
        };

        // Build paragraph groups: each group is a Vec of text_indices indices.
        let mut groups: Vec<Vec<usize>> = vec![vec![text_indices[0]]];

        for (gap_idx, pair) in text_indices.windows(2).enumerate() {
            let is_break =
                gaps[gap_idx] > break_threshold || has_non_text_between(pair[0], pair[1]);

            if is_break {
                // Start a new paragraph group.
                groups.push(vec![pair[1]]);
            } else if let Some(current_group) = groups.last_mut() {
                // Continue the current group.
                current_group.push(pair[1]);
            }
        }

        // Emit Paragraph hint only for the FIRST primitive of each group.
        //
        // The assembler will accumulate subsequent bare text primitives into
        // the same paragraph until a new Paragraph hint (or other structural
        // hint) is encountered. Emitting for every primitive would cause the
        // assembler to flush and start a new paragraph on each one.
        let mut hints = Vec::with_capacity(groups.len());
        for group in &groups {
            if let Some(&first_idx) = group.first() {
                hints.push(DetectedHint {
                    primitive_index: first_idx,
                    hint: SemanticHint::from_heuristic(
                        HintKind::Paragraph,
                        self.confidence,
                        "ParagraphDetector",
                    ),
                });
            }
        }

        hints
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compute the median of a non-empty f32 slice.
fn median(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted: Vec<f32> = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{BoundingBox, Primitive, PrimitiveKind, SemanticHint};
    use crate::structure::detectors::DetectionContext;
    use crate::structure::types::FontHistogram;

    fn text_at(y: f32, height: f32, content: &str, order: u64) -> Primitive {
        Primitive::observed(
            PrimitiveKind::Text {
                content: content.to_string(),
                font_size: 12.0,
                font_name: "Arial".to_string(),
                is_bold: false,
                is_italic: false,
                color: None,
                char_positions: None,
                text_direction: Default::default(),
                text_direction_origin: crate::model::provenance::ValueOrigin::Observed,
                lang: None,
                lang_origin: None,
            },
            BoundingBox::new(0.05, y, 0.9, height),
            0,
            order,
        )
    }

    fn line_at(y: f32, order: u64) -> Primitive {
        use crate::model::Point;
        Primitive::observed(
            PrimitiveKind::Line {
                start: Point::new(0.05, y),
                end: Point::new(0.95, y),
                thickness: 0.001,
                color: None,
            },
            BoundingBox::new(0.05, y, 0.9, 0.001),
            0,
            order,
        )
    }

    fn ctx() -> DetectionContext {
        DetectionContext {
            font_histogram: FontHistogram::default(),
            page: 0,
            page_dims: None,
            text_boxes: None,
            open_table_columns: None,
        }
    }

    #[test]
    fn single_text_gets_paragraph_hint() {
        let page = vec![text_at(0.1, 0.02, "hello", 0)];
        let detector = ParagraphDetector::default();
        let hints = detector.detect(&page, &ctx());

        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].primitive_index, 0);
        assert!(matches!(hints[0].hint.kind, HintKind::Paragraph));
        assert!((hints[0].hint.confidence - 0.7).abs() < 0.01);
    }

    #[test]
    fn consecutive_lines_same_paragraph() {
        // Three text primitives with small Y-gaps (consistent line spacing).
        let page = vec![
            text_at(0.10, 0.02, "line 1", 0),
            text_at(0.13, 0.02, "line 2", 1), // gap: 0.01
            text_at(0.16, 0.02, "line 3", 2), // gap: 0.01
        ];
        let detector = ParagraphDetector::default();
        let hints = detector.detect(&page, &ctx());

        // Only the FIRST primitive of the group gets the Paragraph hint.
        // Subsequent bare prims will merge in the assembler.
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].primitive_index, 0);
        assert!(matches!(hints[0].hint.kind, HintKind::Paragraph));
    }

    #[test]
    fn large_gap_splits_paragraphs() {
        // Two blocks with a large gap between them.
        let page = vec![
            text_at(0.10, 0.02, "para1 line1", 0),
            text_at(0.13, 0.02, "para1 line2", 1), // gap: 0.01
            // Large gap here (0.20 - 0.15 = 0.05 >> median 0.01)
            text_at(0.20, 0.02, "para2 line1", 2),
            text_at(0.23, 0.02, "para2 line2", 3), // gap: 0.01
        ];
        let detector = ParagraphDetector::default();
        let hints = detector.detect(&page, &ctx());

        // Two groups → two Paragraph hints (first prim of each group).
        assert_eq!(hints.len(), 2);
        assert_eq!(hints[0].primitive_index, 0);
        assert_eq!(hints[1].primitive_index, 2);
    }

    #[test]
    fn non_text_breaks_paragraph() {
        let page = vec![
            text_at(0.10, 0.02, "before line", 0),
            line_at(0.13, 1), // Horizontal rule
            text_at(0.14, 0.02, "after line", 2),
        ];
        let detector = ParagraphDetector::default();
        let hints = detector.detect(&page, &ctx());

        // Both text primitives should get Paragraph hints (line doesn't get one).
        assert_eq!(hints.len(), 2);
        assert_eq!(hints[0].primitive_index, 0);
        assert_eq!(hints[1].primitive_index, 2);
    }

    #[test]
    fn already_hinted_primitives_skipped() {
        let page = vec![
            text_at(0.10, 0.02, "hinted", 0)
                .with_hint(SemanticHint::from_format(HintKind::Paragraph)),
            text_at(0.13, 0.02, "unhinted", 1),
        ];
        let detector = ParagraphDetector::default();
        let hints = detector.detect(&page, &ctx());

        // Only the unhinted primitive should get a hint.
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].primitive_index, 1);
    }

    #[test]
    fn empty_page_returns_no_hints() {
        let page: Vec<Primitive> = vec![];
        let detector = ParagraphDetector::default();
        let hints = detector.detect(&page, &ctx());

        assert!(hints.is_empty());
    }

    #[test]
    fn non_text_only_page_returns_no_hints() {
        let page = vec![line_at(0.1, 0), line_at(0.2, 1)];
        let detector = ParagraphDetector::default();
        let hints = detector.detect(&page, &ctx());

        assert!(hints.is_empty());
    }

    #[test]
    fn median_helper_works() {
        assert!((median(&[1.0, 2.0, 3.0]) - 2.0).abs() < 0.001);
        assert!((median(&[1.0, 3.0]) - 2.0).abs() < 0.001);
        assert!((median(&[5.0]) - 5.0).abs() < 0.001);
        assert!((median(&[]) - 0.0).abs() < 0.001);
    }

    #[test]
    fn validate_clamps_invalid_gap_factor() {
        let mut d = ParagraphDetector {
            gap_factor: -1.0,
            ..Default::default()
        };
        let issues = d.validate();
        assert!(!issues.is_empty(), "should report clamped gap_factor");
        assert!((d.gap_factor - 1.5).abs() < f32::EPSILON);
    }

    #[test]
    fn validate_clamps_invalid_confidence() {
        let mut d = ParagraphDetector {
            confidence: 1.5,
            ..Default::default()
        };
        let issues = d.validate();
        assert!(!issues.is_empty());
        assert!((d.confidence - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn validate_accepts_valid_config() {
        let mut d = ParagraphDetector::default();
        let issues = d.validate();
        assert!(issues.is_empty(), "default config should be valid");
    }
}
