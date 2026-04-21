//! Stage 2 & 3: Group lines into text boxes and determine reading order.
//!
//! # Stage 2: Lines → Text Boxes
//!
//! Groups lines based on vertical proximity and horizontal overlap. The
//! grouping criterion: the vertical gap between consecutive lines must be
//! less than `line_overlap_threshold × median_line_height`, and the lines
//! must overlap horizontally (preventing cross-column merging).
//!
//! # Stage 3: Reading Order
//!
//! Text boxes are ordered by a combination of vertical position and
//! column detection. The `boxes_flow` parameter controls the ordering
//! strategy.

#[path = "textbox/grouping.rs"]
mod grouping;
#[path = "textbox/ordering.rs"]
mod ordering;
#[cfg(test)]
#[path = "textbox/tests.rs"]
mod tests;

use super::params::LAParams;
use super::types::{TextBox, TextLine};

/// Group text lines into text boxes based on spatial proximity.
///
/// Dispatches between two algorithms:
/// - For `lines.len() < 50`: uses the greedy O(n²) approach (simpler, for small inputs).
/// - For `lines.len() >= 50`: uses the R-tree spatial index (faster for larger inputs).
///
/// Both use a two-phase approach:
/// 1. **Column clustering**: Group lines that overlap horizontally into
///    the same column cluster. This prevents merging lines from different
///    columns even when they share the same Y coordinate.
/// 2. **Vertical merging**: Within each column cluster, merge consecutive
///    lines (sorted by Y) when the vertical gap is within threshold.
pub fn group_lines_into_boxes(lines: &[TextLine], params: &LAParams) -> Vec<TextBox> {
    if lines.is_empty() {
        return Vec::new();
    }

    if lines.len() < 50 {
        grouping::group_lines_greedy(lines, params)
    } else {
        grouping::group_lines_into_boxes_rtree(lines, params)
    }
}

/// Order text boxes by reading order.
///
/// The ordering depends on `boxes_flow`:
/// - `None` — order by geometric closeness (top-left first).
/// - `Some(flow)` with `flow >= 0` — top-to-bottom bias with column detection.
/// - `Some(flow)` with `flow < 0` — proximity-based ordering.
pub fn order_boxes(mut boxes: Vec<TextBox>, params: &LAParams) -> Vec<TextBox> {
    if boxes.len() <= 1 {
        return boxes;
    }

    match params.boxes_flow {
        None => {
            boxes.sort_by(|a, b| {
                a.bbox
                    .y
                    .total_cmp(&b.bbox.y)
                    .then(a.bbox.x.total_cmp(&b.bbox.x))
            });
            boxes
        }
        Some(flow) if flow >= 0.0 => ordering::order_by_columns(boxes, params),
        Some(_) => {
            let indices: Vec<usize> = (0..boxes.len()).collect();
            let ordered_indices = ordering::order_by_weighted_distance(&indices, &boxes, 0.0);
            ordered_indices
                .into_iter()
                .map(|i| boxes[i].clone())
                .collect()
        }
    }
}
