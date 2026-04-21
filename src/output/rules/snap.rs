//! Grid classification and snapping (stage 3 of the rules pipeline).
//!
//! Maps normalized segments onto a target `cols × rows` grid, classifying
//! each as horizontal / vertical / oblique based purely on its projected
//! extent. Oblique segments are dropped — the ASCII palette only contains
//! `-` and `|`.

use super::normalize::RawSegment;

/// A segment classified as orthogonal to the grid and snapped to cell indices.
/// `col`/`row_*` and `row`/`col_*` are inclusive indices.
///
/// Sub-cell fractional positions are preserved alongside the rounded indices:
///
/// - `exact_row` (Horizontal) and `exact_col` (Vertical) carry the midpoint
///   of the fixed axis before `round()` quantization. A downstream pass uses
///   them to decide which side of a text row to nudge a colliding rule
///   toward — a decision that must come from geometry, not semantics.
/// - `exact_r_lo` and `exact_r_hi` (Vertical) carry the unrounded row
///   coordinates of the vertical's endpoints (top and bottom). When a
///   horizontal rule gets shifted off its original row (to avoid bleeding
///   into a text row), the verticals whose endpoints geometrically coincide
///   with that horizontal must inherit the same shift — otherwise the `|`
///   detaches from the `-` corner and appears to leak one row past the
///   border. See `markdown/grid.rs::adjust_rules` / `spatial/grid.rs`.
///
/// A bare renderer that just draws `-`/`|` can ignore these fields.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum GridRule {
    Horizontal {
        row: usize,
        col_start: usize,
        col_end: usize,
        exact_row: f32,
    },
    Vertical {
        col: usize,
        row_start: usize,
        row_end: usize,
        exact_col: f32,
        exact_r_lo: f32,
        exact_r_hi: f32,
    },
}

/// Snap normalized segments onto a target grid of `target_cols × target_rows`.
///
/// Classification rule (threshold = 0.5 cell):
/// - `Δrows < 0.5` and `Δcols ≥ 0.5` → horizontal.
/// - `Δcols < 0.5` and `Δrows ≥ 0.5` → vertical.
/// - Otherwise (both below 0.5 → point, or both at/above 0.5 → oblique)
///   → dropped.
///
/// Segments partially outside the grid are clipped; segments entirely
/// outside are dropped. Segments that reduce to a single cell after
/// clipping are also dropped (nothing to draw).
pub(crate) fn snap_to_grid(
    segments: &[RawSegment],
    target_cols: usize,
    target_rows: usize,
) -> Vec<GridRule> {
    if target_cols == 0 || target_rows == 0 {
        return Vec::new();
    }
    // Linear map `[0, 1] → [0, max_idx]`: a normalized coordinate of `1.0`
    // lands EXACTLY on the last index, with no clamp needed. The older
    // `y * target_rows` convention required clamping `y = 1.0` from `N` to
    // `N - 1`, which in practice collapsed the bottommost two textual rows
    // into a single grid row whenever the second-bottom row was also within
    // half a cell of the boundary (see the weird_invoice footer bug where
    // `IBAN …` and `BIC …` fused onto the same output line).
    let max_col = target_cols - 1;
    let max_row = target_rows - 1;
    let cols_f = (max_col as f32).max(1.0);
    let rows_f = (max_row as f32).max(1.0);

    let mut rules = Vec::with_capacity(segments.len());
    for seg in segments {
        let d_col = (seg.x1 - seg.x0).abs() * cols_f;
        let d_row = (seg.y1 - seg.y0).abs() * rows_f;

        let is_horizontal = d_row < 0.5 && d_col >= 0.5;
        let is_vertical = d_col < 0.5 && d_row >= 0.5;
        if !is_horizontal && !is_vertical {
            continue;
        }

        if is_horizontal {
            // Fixed axis uses the midpoint — robust to stroke-width and
            // sub-pixel offsets.
            let row_f = (seg.y0 + seg.y1) * 0.5 * rows_f;
            let Some(row) = snap_single(row_f, max_row) else {
                continue;
            };
            let c_lo = seg.x0.min(seg.x1) * cols_f;
            let c_hi = seg.x0.max(seg.x1) * cols_f;
            let Some((col_start, col_end)) = clip_span(c_lo, c_hi, max_col) else {
                continue;
            };
            if col_start == col_end {
                continue;
            }
            rules.push(GridRule::Horizontal {
                row,
                col_start,
                col_end,
                exact_row: row_f,
            });
        } else {
            let col_f = (seg.x0 + seg.x1) * 0.5 * cols_f;
            let Some(col) = snap_single(col_f, max_col) else {
                continue;
            };
            let r_lo = seg.y0.min(seg.y1) * rows_f;
            let r_hi = seg.y0.max(seg.y1) * rows_f;
            let Some((row_start, row_end)) = clip_span(r_lo, r_hi, max_row) else {
                continue;
            };
            if row_start == row_end {
                continue;
            }
            rules.push(GridRule::Vertical {
                col,
                row_start,
                row_end,
                exact_col: col_f,
                exact_r_lo: r_lo,
                exact_r_hi: r_hi,
            });
        }
    }

    rules
}

/// Snap a single cell-space coordinate to `[0, max_idx]`.
///
/// Returns `None` when the coordinate is non-finite or clearly outside the
/// grid (more than half a cell above the top or more than half a cell below
/// the last row). With the linear `y * max_idx` convention, a normalized
/// `1.0` lands exactly on `max_idx`, so no clamp is needed for on-frame
/// rules — but we still accept a small overshoot (up to half a cell past
/// the last row) to absorb floating-point slop from earlier pipeline
/// stages.
fn snap_single(v: f32, max_idx: usize) -> Option<usize> {
    if !v.is_finite() {
        return None;
    }
    let r = v.round();
    // Symmetric half-cell slack on both ends, matching the word placement
    // convention `(y * max_idx).round().min(max_idx)`.
    if r < -0.5 || r > max_idx as f32 + 0.5 {
        return None;
    }
    Some((r.max(0.0) as usize).min(max_idx))
}

/// Clip an inclusive span `[lo, hi]` (in cell units) to `[0, max_idx]`.
///
/// Returns `None` when the span lies entirely outside the grid or is
/// non-finite.
fn clip_span(lo: f32, hi: f32, max_idx: usize) -> Option<(usize, usize)> {
    if !lo.is_finite() || !hi.is_finite() {
        return None;
    }
    let lo_r = lo.round();
    let hi_r = hi.round();
    if hi_r < 0.0 || lo_r > max_idx as f32 {
        return None;
    }
    let lo_c = lo_r.max(0.0) as usize;
    let hi_c = (hi_r.min(max_idx as f32)) as usize;
    if lo_c > hi_c {
        return None;
    }
    Some((lo_c, hi_c))
}
