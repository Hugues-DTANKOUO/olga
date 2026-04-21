//! PDF-coordinate line and rectangle extraction (stage 1 of the rules
//! pipeline).
//!
//! Reads stroked lines and stroked rectangle edges from a PDF page and
//! returns them as [`PdfSegment`]s in raw PDF coordinates (Y-up, points).
//! Classification into horizontal / vertical / oblique is deferred — this
//! stage is purely source-of-truth extraction.

use pdf_oxide::PdfDocument;

/// A line segment in raw PDF coordinates (Y axis pointing up, values in
/// PDF points).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PdfSegment {
    pub(crate) x0: f32,
    pub(crate) y0: f32,
    pub(crate) x1: f32,
    pub(crate) y1: f32,
}

/// Extract stroked line segments from a PDF page in raw PDF coordinates.
///
/// Sources:
/// - `doc.extract_lines(page_idx)` — stroked straight lines.
/// - `doc.extract_rects(page_idx)` — stroked rectangles; each contributes
///   its four edges as independent segments.
///
/// Paths without a stroke (fill-only) are skipped. Classification into
/// horizontal/vertical/oblique is deferred to [`snap_to_grid`](super::snap_to_grid),
/// which needs the target grid resolution.
pub(crate) fn extract_pdf_segments(doc: &mut PdfDocument, page_idx: usize) -> Vec<PdfSegment> {
    let mut segments = Vec::new();

    if let Ok(lines) = doc.extract_lines(page_idx) {
        for line in &lines {
            if !line.has_stroke() {
                continue;
            }
            let b = &line.bbox;
            // For axis-aligned lines, bbox endpoints reproduce the line
            // exactly. For oblique lines the AABB gives a conservative
            // reconstruction; those segments get dropped at classification.
            segments.push(PdfSegment {
                x0: b.x,
                y0: b.y,
                x1: b.x + b.width,
                y1: b.y + b.height,
            });
        }
    }

    if let Ok(rects) = doc.extract_rects(page_idx) {
        for rect in &rects {
            if !rect.has_stroke() {
                continue;
            }
            let b = &rect.bbox;
            let x_lo = b.x;
            let y_lo = b.y;
            let x_hi = b.x + b.width;
            let y_hi = b.y + b.height;

            // Bottom edge
            segments.push(PdfSegment {
                x0: x_lo,
                y0: y_lo,
                x1: x_hi,
                y1: y_lo,
            });
            // Top edge
            segments.push(PdfSegment {
                x0: x_lo,
                y0: y_hi,
                x1: x_hi,
                y1: y_hi,
            });
            // Left edge
            segments.push(PdfSegment {
                x0: x_lo,
                y0: y_lo,
                x1: x_lo,
                y1: y_hi,
            });
            // Right edge
            segments.push(PdfSegment {
                x0: x_hi,
                y0: y_lo,
                x1: x_hi,
                y1: y_hi,
            });
        }
    }

    segments
}

/// Compute the axis-aligned bounding box of a set of segments in PDF
/// coordinates. Returns `None` when the slice is empty or contains only
/// non-finite endpoints.
pub(crate) fn pdf_segments_bounds(segments: &[PdfSegment]) -> Option<(f32, f32, f32, f32)> {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    let mut any = false;
    for s in segments {
        for (x, y) in [(s.x0, s.y0), (s.x1, s.y1)] {
            if !x.is_finite() || !y.is_finite() {
                continue;
            }
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
            any = true;
        }
    }
    if !any {
        return None;
    }
    Some((min_x, min_y, max_x, max_y))
}
