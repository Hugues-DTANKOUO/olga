//! Normalize PDF-space segments into the word-placement frame (stage 2).
//!
//! The caller chooses `(min_x, min_y, cw, ch)` so that segments are
//! expressed in the same `[0, 1]`-ish coordinate frame as placed words.
//! The Y axis is flipped so that `y = 0` is the top of the normalized
//! frame — matching the word-placement convention.

use super::pdf_segments::PdfSegment;

/// A line segment in normalized coordinates (both axes nominally in
/// `[0, 1]` when bounds include rule endpoints; Y flipped so 0 is the top).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RawSegment {
    pub(crate) x0: f32,
    pub(crate) y0: f32,
    pub(crate) x1: f32,
    pub(crate) y1: f32,
}

/// Normalize PDF segments using the same `(min_x, min_y, cw, ch)` bounds
/// used for word placement. The Y axis is flipped so 0 is the top of the
/// normalized frame, matching the word-placement convention.
pub(crate) fn normalize_segments(
    segments: &[PdfSegment],
    min_x: f32,
    min_y: f32,
    cw: f32,
    ch: f32,
) -> Vec<RawSegment> {
    if cw <= 0.0 || ch <= 0.0 || !cw.is_finite() || !ch.is_finite() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(segments.len());
    for s in segments {
        let nx0 = (s.x0 - min_x) / cw;
        let nx1 = (s.x1 - min_x) / cw;
        let ny0 = 1.0 - (s.y0 - min_y) / ch;
        let ny1 = 1.0 - (s.y1 - min_y) / ch;
        if !nx0.is_finite() || !nx1.is_finite() || !ny0.is_finite() || !ny1.is_finite() {
            continue;
        }
        out.push(RawSegment {
            x0: nx0,
            y0: ny0,
            x1: nx1,
            y1: ny1,
        });
    }
    out
}
