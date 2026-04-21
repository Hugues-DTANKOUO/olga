use super::pdf_segments::PdfSegment;
use super::*;

fn seg(x0: f32, y0: f32, x1: f32, y1: f32) -> RawSegment {
    RawSegment { x0, y0, x1, y1 }
}

fn pseg(x0: f32, y0: f32, x1: f32, y1: f32) -> PdfSegment {
    PdfSegment { x0, y0, x1, y1 }
}

// -----------------------------------------------------------------
// snap_to_grid
// -----------------------------------------------------------------

#[test]
fn horizontal_segment_classified_as_horizontal() {
    // cols_f = target_cols - 1 = 99, rows_f = target_rows - 1 = 49.
    // x span: 0.1*99=9.9→10, 0.6*99=59.4→59. y mid: 0.5*49=24.5.
    let out = snap_to_grid(&[seg(0.1, 0.5, 0.6, 0.5)], 100, 50);
    assert_eq!(
        out,
        vec![GridRule::Horizontal {
            row: 25,
            col_start: 10,
            col_end: 59,
            exact_row: 24.5,
        }]
    );
}

#[test]
fn vertical_segment_classified_as_vertical() {
    // cols_f = 99, rows_f = 49. x mid: 0.25*99=24.75→col 25. y span:
    // 0.1*49=4.9→5, 0.9*49=44.1→44.
    let out = snap_to_grid(&[seg(0.25, 0.1, 0.25, 0.9)], 100, 50);
    assert_eq!(
        out,
        vec![GridRule::Vertical {
            col: 25,
            row_start: 5,
            row_end: 44,
            exact_col: 24.75,
            exact_r_lo: 4.9,
            exact_r_hi: 44.1,
        }]
    );
}

#[test]
fn oblique_segment_is_dropped() {
    let out = snap_to_grid(&[seg(0.1, 0.1, 0.9, 0.9)], 100, 50);
    assert!(out.is_empty());
}

#[test]
fn diagonal_nearly_horizontal_within_half_cell_is_horizontal() {
    // Tilted by 0.004 in Y = 0.2 rows on a 50-row grid = < 0.5 cells.
    let out = snap_to_grid(&[seg(0.1, 0.500, 0.9, 0.504)], 100, 50);
    assert_eq!(out.len(), 1);
    assert!(matches!(out[0], GridRule::Horizontal { .. }));
}

#[test]
fn point_segment_is_dropped() {
    let out = snap_to_grid(&[seg(0.3, 0.3, 0.3001, 0.3001)], 100, 50);
    assert!(out.is_empty());
}

#[test]
fn segment_entirely_outside_grid_is_dropped() {
    let out = snap_to_grid(&[seg(0.1, 2.0, 0.9, 2.0)], 100, 50);
    assert!(out.is_empty());
}

#[test]
fn segment_partially_outside_grid_is_clipped() {
    let out = snap_to_grid(&[seg(-0.1, 0.5, 0.5, 0.5)], 100, 50);
    assert_eq!(
        out,
        vec![GridRule::Horizontal {
            row: 25,
            col_start: 0,
            col_end: 50,
            exact_row: 24.5,
        }]
    );
}

#[test]
fn segment_exceeding_grid_is_clipped_to_max() {
    let out = snap_to_grid(&[seg(0.2, 0.5, 2.0, 0.5)], 100, 50);
    assert_eq!(
        out,
        vec![GridRule::Horizontal {
            row: 25,
            col_start: 20,
            col_end: 99,
            exact_row: 24.5,
        }]
    );
}

#[test]
fn empty_grid_returns_no_rules() {
    let out = snap_to_grid(&[seg(0.1, 0.5, 0.9, 0.5)], 0, 50);
    assert!(out.is_empty());
    let out = snap_to_grid(&[seg(0.1, 0.5, 0.9, 0.5)], 100, 0);
    assert!(out.is_empty());
}

#[test]
fn non_finite_segments_are_dropped() {
    let out = snap_to_grid(&[seg(f32::NAN, 0.5, 0.9, 0.5)], 100, 50);
    assert!(out.is_empty());
    let out = snap_to_grid(&[seg(0.1, 0.5, f32::INFINITY, 0.5)], 100, 50);
    assert!(out.is_empty());
}

#[test]
fn horizontal_collapsing_to_single_cell_is_dropped() {
    let out = snap_to_grid(&[seg(0.100, 0.5, 0.103, 0.5)], 100, 50);
    assert!(out.is_empty());
}

#[test]
fn rule_at_bottom_boundary_lands_on_last_row() {
    // When rule bounds coincide with the normalization frame (y = 1.0
    // exactly), the linear `y * (rows_f - 1)` map lands the rule exactly
    // on the last row — no clamp required. This replaces the older
    // `y * rows_f` convention where y=1.0 produced `rows_f` and had to
    // be clamped down, a mapping that collapsed the two bottommost text
    // rows together whenever they both fell inside the clamp window.
    let out = snap_to_grid(&[seg(0.1, 1.0, 0.9, 1.0)], 100, 50);
    assert_eq!(
        out,
        vec![GridRule::Horizontal {
            row: 49,
            col_start: 10,
            col_end: 89,
            exact_row: 49.0,
        }]
    );
}

#[test]
fn rule_at_right_boundary_lands_on_last_col() {
    // Mirror of the above for vertical rules sitting on the right edge.
    let out = snap_to_grid(&[seg(1.0, 0.1, 1.0, 0.9)], 100, 50);
    assert_eq!(
        out,
        vec![GridRule::Vertical {
            col: 99,
            row_start: 5,
            row_end: 44,
            exact_col: 99.0,
            exact_r_lo: 4.9,
            exact_r_hi: 44.1,
        }]
    );
}

#[test]
fn micro_segment_below_half_row_is_dropped_on_calibrated_grid() {
    // Regression guard: a signature-texture mesh in a Quebec-government
    // letter-sized PDF produces hundreds of 0.85pt-tall vertical stroke
    // segments packed tightly. On a 525pt-tall page normalized against
    // the word+rule bounding box, each segment is 0.85/525 ≈ 0.00162 in
    // normalized Y. With an ASCII-calibrated grid — where
    // `target_rows ≈ target_cols * (ch/cw) * 0.5` to make 1 row map to
    // the same visual extent as 2 columns — this becomes
    // 0.00162 * 106 ≈ 0.17 rows, comfortably below the 0.5-row threshold.
    //
    // The opposite regression (target_rows stretched by ~3x the terminal
    // aspect) had d_row ≈ 0.52 → each micro-segment classified as vertical
    // → stacked into spurious `|||||||` barcode-like clusters. We probe
    // at `y = 0.3` (not `0.5`) so that `y * rows_f` stays clear of the
    // half-integer boundary where Rust's `round()` snaps ties upward and
    // hides collapse-after-clip behind classification.
    let h_norm = 0.85 / 525.0;
    let target_cols = 235;
    let target_rows = 106;
    let out = snap_to_grid(
        &[seg(0.5, 0.3, 0.5, 0.3 + h_norm)],
        target_cols,
        target_rows,
    );
    assert!(
        out.is_empty(),
        "0.85pt mesh micro-segment should be dropped on an ASCII-calibrated grid, got {out:?}",
    );
}

#[test]
fn micro_segment_passes_on_overstretched_grid_sanity() {
    // Documents the prior bad behavior as a counterfactual: with
    // `target_rows ≈ target_cols * 1.3` (the pre-calibration formula
    // that ignored the terminal char aspect ratio), the same 0.85pt
    // mesh-pixel crossed the 0.5-cell threshold AND its endpoints
    // landed on different cells, so it was wrongly preserved as a
    // pipe. This test pins that invariant so any future change that
    // reintroduces the overstretched sizing fails both this and the
    // calibrated-grid test in concert.
    let h_norm = 0.85 / 525.0;
    let target_cols = 235;
    let overstretched_target_rows = 321;
    let out = snap_to_grid(
        &[seg(0.5, 0.3, 0.5, 0.3 + h_norm)],
        target_cols,
        overstretched_target_rows,
    );
    assert_eq!(
        out.len(),
        1,
        "sanity: the pre-fix overstretched grid classified mesh pixels as vertical, got {out:?}",
    );
    assert!(matches!(out[0], GridRule::Vertical { .. }));
}

// -----------------------------------------------------------------
// pdf_segments_bounds
// -----------------------------------------------------------------

#[test]
fn empty_segments_have_no_bounds() {
    assert_eq!(pdf_segments_bounds(&[]), None);
}

#[test]
fn bounds_cover_all_endpoints() {
    let b = pdf_segments_bounds(&[pseg(10.0, 20.0, 100.0, 20.0), pseg(50.0, 5.0, 50.0, 200.0)])
        .expect("non-empty");
    assert_eq!(b, (10.0, 5.0, 100.0, 200.0));
}

#[test]
fn bounds_ignore_non_finite_endpoints() {
    let b = pdf_segments_bounds(&[
        pseg(10.0, 20.0, 100.0, 20.0),
        pseg(f32::NAN, f32::NAN, f32::NAN, f32::NAN),
    ])
    .expect("non-empty");
    assert_eq!(b, (10.0, 20.0, 100.0, 20.0));
}

// -----------------------------------------------------------------
// normalize_segments
// -----------------------------------------------------------------

#[test]
fn normalize_flips_y_axis() {
    let pdf = [pseg(100.0, 700.0, 500.0, 700.0)];
    let out = normalize_segments(&pdf, 100.0, 200.0, 400.0, 600.0);
    // y=700 → ny = 1 - (700-200)/600 = 1 - 0.833 = 0.167
    assert_eq!(out.len(), 1);
    assert!((out[0].x0 - 0.0).abs() < 1e-5);
    assert!((out[0].x1 - 1.0).abs() < 1e-5);
    assert!((out[0].y0 - 0.1666667).abs() < 1e-3);
}

#[test]
fn normalize_drops_on_degenerate_bounds() {
    let pdf = [pseg(0.0, 0.0, 1.0, 1.0)];
    assert!(normalize_segments(&pdf, 0.0, 0.0, 0.0, 10.0).is_empty());
    assert!(normalize_segments(&pdf, 0.0, 0.0, 10.0, 0.0).is_empty());
    assert!(normalize_segments(&pdf, 0.0, 0.0, f32::NAN, 10.0).is_empty());
}
