//! Skew-aware line clustering for raw PDF characters.
//!
//! The clustering algorithm is non-expansive (pairwise gap comparison against
//! the previous char only) to prevent a drifting skewed line from absorbing
//! the next line; this matches the convention used by mainstream PDF text
//! extractors (see pdfminer.six in ATTRIBUTION.md) and replaces an earlier
//! elastic [min, max] range that was historically used.

use std::cmp::Ordering;

use crate::formats::pdf::types::*;

use super::direction::is_vertical_rotation;

/// Line-clustering tolerance as a fraction of the character's `font_size`.
///
/// Two chars belong to the same visual line when their baseline y-projections
/// (perpendicular to the estimated skew angle) differ by less than
/// `tolerance = font_size * LINE_CLUSTER_TOLERANCE`.
///
/// Industry reference:
/// - pdfminer.six `line_overlap = 0.5` (overlap ratio, ≈ 0.5·h tolerance)
/// - PDFBox default baseline tolerance ≈ 0.3·font_size
/// - pdfplumber `cluster_objects` tolerance defaults to a few points
///
/// We pick 0.30 (tighter than pdfminer.six's 0.50) because we cluster on
/// **baselines projected onto the page-normal axis** — skew has already
/// been factored out, so a tighter tolerance doesn't punish skewed scans.
///
/// Shared with [`reorder`](super::reorder) so the extract_spans path uses the
/// same geometric threshold.
pub(super) const LINE_CLUSTER_TOLERANCE: f32 = 0.30;

/// Maximum absolute rotation (deg) at which a char is considered horizontal
/// for the purpose of skew estimation. Chars beyond this are either vertical
/// (CJK column text) or outliers (rotated stamps, watermarks).
const SKEW_MAX_ANGLE_DEG: f32 = 15.0;

/// Median absolute deviation (deg) above which we abandon the per-glyph
/// rotation signal and fall back to `theta = 0`. A well-formed skewed scan
/// has near-identical `rotation_degrees` on every glyph; a high MAD means
/// rotations are inconsistent (mixed inline rotated stamps, etc.).
const SKEW_MAD_MAX_DEG: f32 = 2.0;

/// Minimum number of horizontal chars required to trust the rotation median.
/// Below this, small samples are noisy and we default to `theta = 0`.
const SKEW_MIN_SAMPLE: usize = 3;

/// Cluster raw chars into visual lines, order the lines ascending by
/// baseline (preserving the historical `y` ordering), and sort chars within
/// each line by `x` (ascending) so that even skewed or rotated-scan pages
/// yield the correct reading order.
///
/// Vertical rotated chars (90°/270°) are kept together by `raw_x` proximity
/// instead of `raw_y` overlap because their advance axis is Y, not X.
pub(super) fn cluster_chars_into_lines(chars: &[RawChar]) -> Vec<&RawChar> {
    if chars.is_empty() {
        return Vec::new();
    }

    // Separate vertical-rotation chars; they cluster along the X axis instead.
    let (vertical, horizontal): (Vec<&RawChar>, Vec<&RawChar>) = chars
        .iter()
        .partition(|c| is_vertical_rotation(c.rotation_degrees));

    let mut out: Vec<&RawChar> = Vec::with_capacity(chars.len());
    out.extend(order_horizontal_chars_by_line(&horizontal));
    out.extend(order_vertical_chars_by_column(&vertical));
    out
}

/// Estimate the page's text-baseline angle from the PDF text matrix encoded in
/// each character's `rotation_degrees`.
///
/// Why this works: `rotation_degrees = atan2(b, a)` where `(a, b, c, d)` is
/// the final text matrix pdf_oxide applied to the glyph. A producer placing
/// skewed text uses the same rotation across every glyph of that region, so
/// the median is the true skew — no geometric estimation needed.
///
/// Returns 0 when:
/// - too few samples (< `SKEW_MIN_SAMPLE`)
/// - rotations are inconsistent (MAD > `SKEW_MAD_MAX_DEG`)
/// - magnitude outside ±`SKEW_MAX_ANGLE_DEG` (treated as vertical/outlier)
fn estimate_line_angle_radians(chars: &[&RawChar]) -> f32 {
    let angles_deg: Vec<f32> = chars
        .iter()
        .map(|c| {
            // Normalize to [-180, 180] then keep only near-horizontal range.
            let r = c.rotation_degrees.rem_euclid(360.0);
            if r > 180.0 { r - 360.0 } else { r }
        })
        .filter(|r| r.abs() <= SKEW_MAX_ANGLE_DEG)
        .collect();

    if angles_deg.len() < SKEW_MIN_SAMPLE {
        return 0.0;
    }

    let median = median_f32(&angles_deg);
    let mad = median_abs_deviation(&angles_deg, median);
    if mad > SKEW_MAD_MAX_DEG {
        return 0.0;
    }
    median.to_radians()
}

/// Project a 2D point into the line frame: new X along baselines, new Y
/// perpendicular to baselines. Identity when `theta_rad ≈ 0`.
#[inline]
fn project_baseline(x: f32, y: f32, theta_rad: f32) -> (f32, f32) {
    if theta_rad.abs() < 1e-5 {
        return (x, y);
    }
    let (s, c) = theta_rad.sin_cos();
    // Forward rotation by -theta: inverts the skew so lines become horizontal
    // in the projected frame.
    let x_proj = x * c + y * s;
    let y_proj = -x * s + y * c;
    (x_proj, y_proj)
}

/// Baseline reference point for a char: origin_x/origin_y when available
/// (typographic baseline), otherwise the bbox bottom-left (y-up).
#[inline]
fn baseline_point(ch: &RawChar) -> (f32, f32) {
    if ch.origin_x == 0.0 && ch.origin_y == 0.0 {
        (ch.raw_x, ch.raw_y)
    } else {
        (ch.origin_x, ch.origin_y)
    }
}

fn median_f32(xs: &[f32]) -> f32 {
    if xs.is_empty() {
        return 0.0;
    }
    let mut sorted: Vec<f32> = xs.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    let n = sorted.len();
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    }
}

fn median_abs_deviation(xs: &[f32], med: f32) -> f32 {
    let deviations: Vec<f32> = xs.iter().map(|x| (x - med).abs()).collect();
    median_f32(&deviations)
}

/// Industry-standard line clustering for horizontal text.
///
/// Algorithm (aligned with the conventions used by pdfminer.six, PDFBox,
/// and MuPDF):
///   1. Estimate the page's baseline angle θ from the median of each
///      glyph's `rotation_degrees` (directly read from the PDF text matrix
///      via pdf_oxide — this is the true skew, not an estimate from
///      geometry). Fallback to 0 when the signal is inconsistent.
///   2. Project each char's baseline point onto (along-line, perpendicular)
///      axes. For axis-aligned pages this is the identity.
///   3. Sort by perpendicular (`y_proj`) descending — PDF y-up convention
///      means the *top* of the page has the largest y. This is the
///      top-to-bottom reading order that every PDF text extractor uses.
///   4. Cluster into lines with **pairwise** gap comparison, using a fixed
///      tolerance of `LINE_CLUSTER_TOLERANCE · font_size`. Critically, we
///      do NOT expand an elastic [min, max] range as chars accumulate —
///      that was the old bug where a skewed line's drifting bbox would
///      eventually overlap the next line and swallow it, producing
///      character-scrambled output.
///   5. Within each line, sort by along-line (`x_proj`) ascending (LTR).
///      RTL reversal happens later in `ordered_chars_for_span` once the
///      script is known.
fn order_horizontal_chars_by_line<'a>(chars: &[&'a RawChar]) -> Vec<&'a RawChar> {
    if chars.is_empty() {
        return Vec::new();
    }

    // 1. Estimate skew angle from the actual per-glyph text-matrix rotation.
    let theta = estimate_line_angle_radians(chars);

    // 2. Project every char into the skew-corrected frame.
    //    Keep the &RawChar alongside so we don't have to recompute later.
    let mut projected: Vec<(f32, f32, &RawChar)> = chars
        .iter()
        .map(|&ch| {
            let (bx, by) = baseline_point(ch);
            let (xp, yp) = project_baseline(bx, by, theta);
            (xp, yp, ch)
        })
        .collect();

    // 3. Sort by y_proj DESCENDING (top-of-page first). Tiebreak on x_proj
    //    ascending so deterministic ordering within a line at this stage
    //    (re-sorted per-line in step 5 anyway).
    projected.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal))
    });

    // 4. Cluster into lines via pairwise gap comparison. Each comparison is
    //    against the **previous** char only (obj0 ↔ obj1, mirroring the
    //    pairwise scheme used by pdfminer.six), so no elastic range can
    //    absorb an adjacent line.
    //
    //    After projection, same-line chars have y_proj values clustered
    //    tightly around the baseline; between-line chars have a large gap.
    let mut lines: Vec<Vec<(f32, f32, &RawChar)>> = Vec::new();
    let mut cur: Vec<(f32, f32, &RawChar)> = Vec::new();
    let mut prev_y: f32 = 0.0;
    let mut prev_font: f32 = 0.0;

    for item in projected {
        let (_, yp, ch) = item;
        let font = ch.font_size.max(1.0);

        let same_line = if cur.is_empty() {
            true
        } else {
            // Tolerance uses the **larger** font_size of the pair, so a
            // tiny subscript next to a body-text char still joins.
            let ref_font = font.max(prev_font);
            let tol = ref_font * LINE_CLUSTER_TOLERANCE;
            (prev_y - yp).abs() <= tol
        };

        if same_line {
            cur.push(item);
        } else {
            lines.push(std::mem::take(&mut cur));
            cur.push(item);
        }
        prev_y = yp;
        prev_font = font;
    }
    if !cur.is_empty() {
        lines.push(cur);
    }

    // 5. Within each line, sort by x_proj ascending (LTR). The skew-corrected
    //    axis means "along-the-line" order, which is the natural reading
    //    direction regardless of page skew.
    for line in &mut lines {
        line.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));
    }

    lines
        .into_iter()
        .flat_map(|line| line.into_iter().map(|(_, _, c)| c))
        .collect()
}

/// Cluster rotated-90°/-270° chars into vertical columns (CJK top-to-bottom
/// with columns typically running right-to-left).
///
/// Mirror of the horizontal path with X/Y swapped: columns are clustered by
/// pairwise X-gap comparison (no elastic range), within each column chars are
/// ordered by descending `raw_y` so higher-on-page chars (y-up) come first.
fn order_vertical_chars_by_column<'a>(chars: &[&'a RawChar]) -> Vec<&'a RawChar> {
    if chars.is_empty() {
        return Vec::new();
    }

    // Sort by X ascending (columns in left-to-right order on the page).
    let mut by_x: Vec<&RawChar> = chars.to_vec();
    by_x.sort_by(|a, b| a.raw_x.partial_cmp(&b.raw_x).unwrap_or(Ordering::Equal));

    let mut columns: Vec<Vec<&RawChar>> = Vec::new();
    let mut cur_col: Vec<&RawChar> = Vec::new();
    let mut prev_x: f32 = 0.0;
    let mut prev_font: f32 = 0.0;

    for ch in by_x {
        let font = ch.font_size.max(1.0);
        let ch_x = ch.raw_x;

        let same_col = if cur_col.is_empty() {
            true
        } else {
            let ref_font = font.max(prev_font);
            let tol = ref_font * LINE_CLUSTER_TOLERANCE;
            (prev_x - ch_x).abs() <= tol
        };

        if same_col {
            cur_col.push(ch);
        } else {
            columns.push(std::mem::take(&mut cur_col));
            cur_col.push(ch);
        }
        prev_x = ch_x;
        prev_font = font;
    }
    if !cur_col.is_empty() {
        columns.push(cur_col);
    }

    for col in &mut columns {
        // Descending raw_y so that higher-on-page chars (PDF y-up) come first.
        col.sort_by(|a, b| b.raw_y.partial_cmp(&a.raw_y).unwrap_or(Ordering::Equal));
    }

    columns.into_iter().flatten().collect()
}
