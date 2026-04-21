//! Stage 1: Group text spans into text lines.
//!
//! Characters (text spans) are grouped into horizontal lines based on
//! Y-coordinate overlap. Within each line, spans are sorted left-to-right.
//!
//! # Algorithm
//!
//! 1. Sort spans by Y coordinate (top to bottom).
//! 2. Iterate through spans, accumulating them into the current line
//!    as long as the Y-overlap exceeds the `line_margin` threshold.
//! 3. When a span falls below the threshold, flush the current line
//!    and start a new one.
//!
//! The Y-overlap test: two spans are on the same line if their vertical
//! overlap exceeds `(1.0 - line_margin) × min_height`, where `min_height`
//! is the smaller of the two spans' heights. The `line_margin` parameter
//! and the overlap formula are informed by pdfminer.six's `layout.py`
//! (MIT) — see [ATTRIBUTION.md]. The X-gap column-separation heuristic
//! below is Olga's extension.

use super::params::LAParams;
use super::types::{TextLine, TextSpan};

/// Group text spans into horizontal text lines.
///
/// Spans are grouped by Y-coordinate overlap using the `line_margin`
/// parameter. Within each line, spans are sorted left-to-right.
///
/// Returns lines sorted top-to-bottom.
pub fn group_spans_into_lines(spans: &[TextSpan], params: &LAParams) -> Vec<TextLine> {
    if spans.is_empty() {
        return Vec::new();
    }

    // Sort spans by Y coordinate (top to bottom), then X (left to right).
    let mut sorted: Vec<TextSpan> = spans.to_vec();
    sorted.sort_by(|a, b| {
        a.bbox
            .y
            .total_cmp(&b.bbox.y)
            .then(a.bbox.x.total_cmp(&b.bbox.x))
    });

    let mut lines: Vec<TextLine> = Vec::new();
    let mut current_line_spans: Vec<TextSpan> = vec![sorted[0].clone()];

    for span in sorted.iter().skip(1) {
        // Check if this span belongs on the current line.
        // We test against the last span added to the current line group.
        if is_same_line(&current_line_spans, span, params.line_margin) {
            current_line_spans.push(span.clone());
        } else {
            // Flush current line, start new one.
            lines.push(TextLine::from_spans(current_line_spans));
            current_line_spans = vec![span.clone()];
        }
    }

    // Flush final line.
    if !current_line_spans.is_empty() {
        lines.push(TextLine::from_spans(current_line_spans));
    }

    // Sort lines top-to-bottom.
    lines.sort_by(|a, b| a.bbox.y.total_cmp(&b.bbox.y));

    lines
}

/// Check if a span belongs on the same line as the current group.
///
/// Uses two criteria:
/// 1. **Y-overlap**: the vertical overlap between the span
///    and the line's vertical extent must exceed `(1.0 - line_margin) × min_height`.
/// 2. **X-gap** (column separation): if the horizontal gap between the
///    rightmost span in the current line and the candidate exceeds 3× the
///    average span width, they are in different columns and should not be
///    on the same line. This prevents merging text from different columns
///    that happen to share the same Y coordinate.
fn is_same_line(current_spans: &[TextSpan], candidate: &TextSpan, line_margin: f32) -> bool {
    // Compute the Y-extent of the current line from all spans in it.
    let line_y_min = current_spans
        .iter()
        .map(|s| s.bbox.y)
        .fold(f32::MAX, f32::min);
    let line_y_max = current_spans
        .iter()
        .map(|s| s.bbox.y + s.bbox.height)
        .fold(f32::MIN, f32::max);
    let line_height = line_y_max - line_y_min;

    let cand_y_min = candidate.bbox.y;
    let cand_y_max = candidate.bbox.y + candidate.bbox.height;
    let cand_height = candidate.bbox.height;

    // Vertical overlap between the line extent and the candidate.
    let overlap_top = line_y_min.max(cand_y_min);
    let overlap_bottom = line_y_max.min(cand_y_max);
    let overlap = (overlap_bottom - overlap_top).max(0.0);

    // The threshold: overlap must exceed (1.0 - margin) × min_height.
    let min_height = line_height.min(cand_height);
    if min_height <= 0.0 {
        return false;
    }

    let threshold = (1.0 - line_margin) * min_height;
    if overlap < threshold {
        return false;
    }

    // X-gap check: prevent merging spans from different columns.
    // If the horizontal gap between the rightmost edge of the current line
    // and the left edge of the candidate exceeds a threshold, they are in
    // different columns.
    let line_x_max = current_spans
        .iter()
        .map(|s| s.bbox.x + s.bbox.width)
        .fold(f32::MIN, f32::max);
    let x_gap = candidate.bbox.x - line_x_max;

    if x_gap > 0.0 {
        // Use line height as the reference unit: a gap exceeding 5× the
        // average span height indicates a column boundary. This is robust
        // because normal word spacing is ~0.3–0.5× height, while column
        // gaps are typically 3–10× height.
        let avg_height =
            current_spans.iter().map(|s| s.bbox.height).sum::<f32>() / current_spans.len() as f32;
        if x_gap > avg_height * 5.0 {
            return false;
        }
    }

    true
}
