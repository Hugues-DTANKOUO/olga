use crate::model::Primitive;

use super::barriers::{collect_vertical_barriers, has_barrier_between, y_overlap_ratio};
use super::{AlignedSpan, PageMetrics, VisualLine};

/// Group a page's primitives into visual lines.
///
/// Only considers text primitives. Non-text primitives (lines, images,
/// rectangles) are left untouched for other detectors to handle.
///
/// Returns visual lines sorted top-to-bottom by their topmost Y coordinate.
pub fn group_visual_lines(primitives: &[Primitive], metrics: &PageMetrics) -> Vec<VisualLine> {
    let mut text_prims: Vec<(usize, &Primitive)> = primitives
        .iter()
        .enumerate()
        .filter(|(_, p)| p.is_text() && p.text_content().is_some_and(|t| !t.trim().is_empty()))
        .collect();

    if text_prims.is_empty() {
        return Vec::new();
    }

    let vertical_barriers = collect_vertical_barriers(primitives);

    text_prims.sort_by(|a, b| {
        a.1.bbox
            .y
            .total_cmp(&b.1.bbox.y)
            .then_with(|| a.1.bbox.x.total_cmp(&b.1.bbox.x))
    });

    let mut lines: Vec<VisualLine> = Vec::new();

    for (idx, prim) in &text_prims {
        let mut merged = false;

        for line in lines.iter_mut().rev() {
            if y_overlap_ratio(&line.bbox, &prim.bbox) > 0.5 {
                if has_barrier_between(line, &prim.bbox, &vertical_barriers) {
                    break;
                }
                let text = prim.text_content().unwrap_or("").to_string();
                let col_start = (prim.bbox.x / metrics.avg_char_width).round() as usize;
                line.spans.push(AlignedSpan {
                    col_start,
                    text,
                    prim_index: *idx,
                    bbox: prim.bbox,
                });
                line.prim_indices.push(*idx);
                line.bbox = line.bbox.merge(&prim.bbox);
                merged = true;
                break;
            }

            if prim.bbox.y > line.bbox.bottom() + line.bbox.height {
                break;
            }
        }

        if !merged {
            let text = prim.text_content().unwrap_or("").to_string();
            let col_start = (prim.bbox.x / metrics.avg_char_width).round() as usize;
            lines.push(VisualLine {
                spans: vec![AlignedSpan {
                    col_start,
                    text,
                    prim_index: *idx,
                    bbox: prim.bbox,
                }],
                bbox: prim.bbox,
                prim_indices: vec![*idx],
            });
        }
    }

    for line in &mut lines {
        line.spans.sort_by(|a, b| a.bbox.x.total_cmp(&b.bbox.x));
        recompute_columns(line, metrics);
    }

    lines.sort_by(|a, b| a.bbox.y.total_cmp(&b.bbox.y));

    lines
}

/// Recompute character columns for all spans in a line, ensuring no overlaps.
fn recompute_columns(line: &mut VisualLine, metrics: &PageMetrics) {
    let mut cursor: usize = 0;

    for span in &mut line.spans {
        let ideal_col = (span.bbox.x / metrics.avg_char_width).round() as usize;

        span.col_start = if ideal_col > cursor {
            ideal_col
        } else {
            cursor
        };

        let col_width = (span.bbox.width / metrics.avg_char_width).round() as usize;
        let char_count = span.text.chars().count();
        cursor = span.col_start + col_width.max(char_count);
        cursor += 1;
    }
}

/// Render a visual line as a proportionally-spaced string.
pub fn render_line(line: &VisualLine) -> String {
    let mut out = String::new();
    let mut cursor: usize = 0;

    for span in &line.spans {
        if span.col_start > cursor {
            for _ in 0..(span.col_start - cursor) {
                out.push(' ');
            }
        }
        out.push_str(&span.text);
        cursor = span.col_start + span.text.chars().count();
    }

    out.trim_end().to_string()
}
