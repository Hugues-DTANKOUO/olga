//! List item detector — identifies list items from bullet/number prefixes
//! and indentation patterns.
//!
//! # Algorithm
//!
//! 1. Scan text primitives in reading order for bullet-like prefixes:
//!    - Unordered markers: `•`, `–`, `—`, `◦`, `▪`, `‣`, `-`, `*`, `○`
//!    - Ordered markers: `1.`, `2)`, `a.`, `(iv)`, etc.
//! 2. Validate with indentation: items with a common indent level that
//!    are separated by consistent Y-gaps reinforce the list hypothesis.
//! 3. Assign `ListItem` hints with depth=0 for top-level items.
//!    Nested lists (depth > 0) are detected by deeper indentation relative
//!    to the first item in the group.
//!
//! # Limitations
//!
//! - Cannot distinguish definition lists from bullet lists.
//! - Does not detect continuation paragraphs within a list item
//!   (those would need heuristic indentation matching).
//! - Depth detection beyond 2 levels is unreliable in PDF since
//!   indent spacing varies widely between typesetting tools.

#[path = "list/detect.rs"]
mod detect;
#[path = "list/helpers.rs"]
mod helpers;
#[path = "list/prefix.rs"]
mod prefix;
#[cfg(test)]
#[path = "list/tests.rs"]
mod tests;

use crate::model::Primitive;

use super::{DetectedHint, DetectionContext, StructureDetector};

/// Detects list items based on bullet prefixes and indentation patterns.
///
/// Confidence: 0.75 for items with recognized bullet/number prefix.
/// Lower confidence (0.55) for items detected by indent-only heuristic.
pub struct ListDetector {
    /// Confidence for items with an explicit bullet or number prefix.
    /// Default: 0.75.
    pub prefix_confidence: f32,
    /// Confidence for items detected by indentation alone (no bullet).
    /// Default: 0.55.
    pub indent_confidence: f32,
    /// Minimum X-offset (in normalized coords) to consider an indent
    /// relative to the page's dominant left margin. Default: 0.02.
    pub min_indent: f32,
    /// Indent step (in normalized coords) to detect nested list levels.
    /// Default: 0.03.
    pub indent_step: f32,
    /// Enable indent-only list detection (no bullet/number prefix required).
    /// When enabled, sequences of 3+ items at the same indent level are
    /// emitted as ListItem hints with `indent_confidence`. Default: false.
    pub enable_indent_only: bool,
    /// Enable definition list detection (term: definition pattern).
    /// When enabled, sequences of 3+ items matching the pattern are
    /// emitted as ListItem hints with `indent_confidence`. Default: false.
    pub enable_definition_lists: bool,
}

impl Default for ListDetector {
    fn default() -> Self {
        Self {
            prefix_confidence: 0.75,
            indent_confidence: 0.55,
            min_indent: 0.02,
            indent_step: 0.03,
            enable_indent_only: false,
            enable_definition_lists: false,
        }
    }
}

impl ListDetector {
    /// Validate configuration values and clamp to sensible ranges.
    pub fn validate(&mut self) -> Vec<String> {
        let mut issues = Vec::new();

        if self.prefix_confidence < 0.0 || self.prefix_confidence > 1.0 {
            issues.push(format!(
                "ListDetector: prefix_confidence {} clamped to 0.75",
                self.prefix_confidence
            ));
            self.prefix_confidence = 0.75;
        }
        if self.indent_confidence < 0.0 || self.indent_confidence > 1.0 {
            issues.push(format!(
                "ListDetector: indent_confidence {} clamped to 0.55",
                self.indent_confidence
            ));
            self.indent_confidence = 0.55;
        }
        if self.min_indent <= 0.0 {
            issues.push(format!(
                "ListDetector: min_indent {} clamped to 0.02",
                self.min_indent
            ));
            self.min_indent = 0.02;
        }

        issues
    }
}

impl StructureDetector for ListDetector {
    fn name(&self) -> &str {
        "list"
    }

    fn detect(&self, page: &[Primitive], context: &DetectionContext) -> Vec<DetectedHint> {
        let mut hints = if let Some(ref text_boxes) = context.text_boxes {
            self.detect_from_boxes(page, text_boxes)
        } else {
            self.detect_from_primitives(page)
        };

        Self::assign_group_ids(&mut hints, page);
        hints
    }
}
