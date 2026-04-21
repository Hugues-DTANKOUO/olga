use crate::model::{BoundingBox, Primitive, PrimitiveKind};

use super::VisualLine;

/// A vertical barrier: a vertical line segment that can separate columns.
#[derive(Debug)]
pub(super) struct VerticalBarrier {
    /// X position of the vertical line.
    pub(super) x: f32,
    /// Top Y of the line.
    pub(super) y_top: f32,
    /// Bottom Y of the line.
    pub(super) y_bottom: f32,
}

/// Collect vertical line barriers from all Line and Rectangle primitives.
///
/// A vertical barrier is a line/rectangle that is much taller than wide
/// (height > 3× width) — i.e., a vertical stroke or border.
pub(super) fn collect_vertical_barriers(primitives: &[Primitive]) -> Vec<VerticalBarrier> {
    let mut barriers = Vec::new();

    for prim in primitives {
        match &prim.kind {
            PrimitiveKind::Line { .. } | PrimitiveKind::Rectangle { .. } => {
                let b = &prim.bbox;
                if b.height > b.width * 3.0 && b.height > 0.02 {
                    barriers.push(VerticalBarrier {
                        x: b.x + b.width / 2.0,
                        y_top: b.y,
                        y_bottom: b.y + b.height,
                    });
                }
            }
            _ => {}
        }
    }

    barriers
}

/// Check if a vertical barrier exists between a visual line's rightmost
/// content and a new primitive being considered for merging.
pub(super) fn has_barrier_between(
    line: &VisualLine,
    new_prim_bbox: &BoundingBox,
    barriers: &[VerticalBarrier],
) -> bool {
    let line_right = line
        .spans
        .iter()
        .map(|s| s.bbox.x + s.bbox.width)
        .fold(0.0f32, f32::max);
    let new_left = new_prim_bbox.x;

    if new_left <= line_right {
        return false;
    }

    let y_mid = (line.bbox.y + line.bbox.y + line.bbox.height) / 2.0;

    for barrier in barriers {
        if barrier.x > line_right
            && barrier.x < new_left
            && barrier.y_top <= y_mid
            && barrier.y_bottom >= y_mid
        {
            return true;
        }
    }

    false
}

/// Compute the overlap ratio between two bounding boxes along the Y axis.
///
/// Returns the overlap height divided by the shorter box's height.
/// Returns 0.0 if there is no overlap.
pub(super) fn y_overlap_ratio(a: &BoundingBox, b: &BoundingBox) -> f32 {
    let overlap_top = a.y.max(b.y);
    let overlap_bottom = a.bottom().min(b.bottom());
    let overlap = (overlap_bottom - overlap_top).max(0.0);

    let min_height = a.height.min(b.height);
    if min_height <= 0.0 {
        return 0.0;
    }

    overlap / min_height
}
