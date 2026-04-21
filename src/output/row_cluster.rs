//! Cluster sub-cell y-jitter so that words sharing a PDF baseline survive
//! row quantization.
//!
//! # Why
//!
//! The grid renderer maps normalized y coordinates to integer rows via
//! `round(ny * (target_rows - 1))`. When two words lie on the same visual
//! line but carry slightly different PDF baselines (different labels, text
//! boxes, or glyph baseline offsets), their unrounded positions can fall on
//! opposite sides of an integer `+ 0.5` boundary and split onto adjacent
//! rows — even though a human reader sees one line.
//!
//! The old `ny * target_rows` mapping masked some of this at the bottom of
//! the page because every position past `target_rows - 0.5` got clamped to
//! `target_rows - 1`, piling many distinct baselines onto the same row.
//! That side effect fixed some collisions by accident — and created others
//! (e.g. the `weird_invoice` footer where two real lines fused). The
//! replacement `ny * (target_rows - 1)` mapping is geometrically correct
//! but surfaces the baseline jitter anywhere on the page.
//!
//! This module recovers the "same line stays same row" invariant without
//! the bottom-row pile-up, by explicitly clustering exact row positions
//! within a sub-cell tolerance before rounding.
//!
//! # Algorithm
//!
//! Sweep the values in ascending order. A new item joins the current
//! cluster when its distance from the cluster's **start** is at most
//! `TOL_CELL`. Bounding the cluster width prevents single-linkage chaining
//! from fusing four adjacent lines when each consecutive pair is within
//! tolerance; the cluster can never grow beyond `TOL_CELL` cells wide, so
//! a natural ≥1-cell separation between real rows always breaks the chain.
//!
//! Every cluster member is then replaced by the cluster's arithmetic mean
//! — the most stable canonical position (median-like under tight jitter,
//! symmetric under floating-point noise).

/// Maximum sub-cell jitter to merge, in cells (i.e. in units of
/// `target_rows` fractional positions). Chosen strictly below `0.5` so
/// that two positions separated by `≥ 1` full cell — the natural spacing
/// of genuinely distinct lines — never end up in the same cluster even
/// under worst-case chaining.
const TOL_CELL: f32 = 0.45;

/// Snap each position in `exact` to the arithmetic mean of its cluster.
///
/// `exact[i]` is the sub-cell fractional row for item `i` (typically
/// `ny_i * (target_rows - 1)`). Items whose exact positions agree within
/// `TOL_CELL` cells of each other become a cluster and are rewritten to
/// the cluster's mean. Returns a `Vec<f32>` parallel to `exact`.
///
/// Stable with respect to input order: the output at index `i` depends
/// only on the full set of values, not on their order in the slice.
/// Cluster width is capped at `TOL_CELL` even for long chains — four
/// items at positions `0.0, 0.4, 0.8, 1.2` produce two clusters (at
/// `0.2` and `1.0`), not one at `0.6`.
pub(crate) fn snap_same_row(exact: &[f32]) -> Vec<f32> {
    if exact.is_empty() {
        return Vec::new();
    }

    let mut order: Vec<usize> = (0..exact.len()).collect();
    order.sort_by(|a, b| exact[*a].total_cmp(&exact[*b]));

    let mut canonical = vec![0.0f32; exact.len()];
    let mut members: Vec<usize> = Vec::new();
    let mut cluster_start: f32 = 0.0;

    let flush = |members: &[usize], exact: &[f32], canonical: &mut [f32]| {
        if members.is_empty() {
            return;
        }
        let mean = members.iter().map(|&i| exact[i]).sum::<f32>() / members.len() as f32;
        for &i in members {
            canonical[i] = mean;
        }
    };

    for &i in &order {
        let v = exact[i];
        if members.is_empty() {
            cluster_start = v;
            members.push(i);
        } else if v - cluster_start <= TOL_CELL {
            members.push(i);
        } else {
            flush(&members, exact, &mut canonical);
            members.clear();
            cluster_start = v;
            members.push(i);
        }
    }
    flush(&members, exact, &mut canonical);

    canonical
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn empty_in_empty_out() {
        assert!(snap_same_row(&[]).is_empty());
    }

    #[test]
    fn single_value_round_trips() {
        let got = snap_same_row(&[7.3]);
        assert_eq!(got.len(), 1);
        assert!(approx(got[0], 7.3));
    }

    #[test]
    fn isolated_values_unchanged() {
        // Two positions separated by ≥ 1 cell stay distinct.
        let got = snap_same_row(&[3.0, 5.0]);
        assert!(approx(got[0], 3.0));
        assert!(approx(got[1], 5.0));
    }

    #[test]
    fn close_values_snap_to_mean() {
        // Jitter of 0.1 cell around a baseline merges.
        let got = snap_same_row(&[10.0, 10.1, 10.2]);
        let mean = (10.0 + 10.1 + 10.2) / 3.0;
        for g in &got {
            assert!(approx(*g, mean), "{} != {}", g, mean);
        }
    }

    #[test]
    fn chain_does_not_fuse_distinct_rows() {
        // Four positions 0.4 apart: single-linkage WOULD chain them all.
        // Width cap at 0.45 must break the chain into two clusters.
        let got = snap_same_row(&[0.0, 0.4, 0.8, 1.2]);
        // {0.0, 0.4} and {0.8, 1.2} — two clusters of two.
        assert!(approx(got[0], got[1]));
        assert!(approx(got[2], got[3]));
        assert!(!approx(got[0], got[2]));
        assert!(approx(got[0], 0.2));
        assert!(approx(got[2], 1.0));
    }

    #[test]
    fn boundary_of_tolerance() {
        // Exactly TOL_CELL apart is INSIDE the cluster (inclusive bound).
        let got = snap_same_row(&[0.0, TOL_CELL]);
        let mean = TOL_CELL / 2.0;
        assert!(approx(got[0], mean));
        assert!(approx(got[1], mean));
    }

    #[test]
    fn just_past_tolerance_splits() {
        // Any gap strictly greater than TOL_CELL → two clusters.
        let got = snap_same_row(&[0.0, TOL_CELL + 0.01]);
        assert!(approx(got[0], 0.0));
        assert!(approx(got[1], TOL_CELL + 0.01));
    }

    #[test]
    fn input_order_does_not_matter() {
        let a = snap_same_row(&[5.0, 5.1, 8.0, 8.2]);
        let b = snap_same_row(&[8.2, 5.0, 8.0, 5.1]);
        // Output is parallel to input; rebuild sorted sets to compare.
        let mut sa: Vec<f32> = a.clone();
        let mut sb: Vec<f32> = b.clone();
        sa.sort_by(f32::total_cmp);
        sb.sort_by(f32::total_cmp);
        for (x, y) in sa.iter().zip(sb.iter()) {
            assert!(approx(*x, *y));
        }
    }

    #[test]
    fn real_world_jitter_around_bottom() {
        // IBAN-like pair: ~1.05 cell apart at the bottom — must NOT merge.
        let got = snap_same_row(&[130.0, 131.05]);
        assert!(approx(got[0], 130.0));
        assert!(approx(got[1], 131.05));
    }

    #[test]
    fn real_world_jitter_same_line() {
        // Rheinstrasse/Issued-like pair: 0.1 cell apart — must merge.
        let got = snap_same_row(&[5.45, 5.55]);
        assert!(approx(got[0], got[1]));
        assert!(approx(got[0], 5.5));
    }
}
