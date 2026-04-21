//! Line-clustering reorder over already-grouped [`TextSpan`]s.
//!
//! Used by the `extract_spans` fallback path where we have no per-glyph
//! rotation signal (pdf_oxide `TextSpan::to_chars` sets
//! `rotation_degrees = 0.0`), so skew estimation isn't available here.

use std::cmp::Ordering;

use crate::formats::pdf::types::*;
use crate::model::*;

use super::clustering::LINE_CLUSTER_TOLERANCE;

/// Reorder already-grouped [`TextSpan`]s (as emitted by `extract_spans`) into
/// visual reading order: top-to-bottom, then left-to-right within each line.
///
/// We still correct the two bugs that affected the extract_chars path:
///   - Non-expansive clustering (pairwise gap check against previous span
///     only, no elastic [min, max] range that could absorb adjacent lines).
///   - Top-first emission: bboxes arrive in y-DOWN normalised coords, so
///     ascending `bbox.y` is top-first as desired.
///
/// Vertical (TopToBottom) spans keep their original position slots so CJK
/// vertical columns are not disturbed.
pub(crate) fn reorder_spans_by_lines(spans: Vec<TextSpan>) -> Vec<TextSpan> {
    if spans.len() <= 1 {
        return spans;
    }

    let indexed: Vec<(usize, TextSpan)> = spans.into_iter().enumerate().collect();
    let horizontal_idx: Vec<usize> = indexed
        .iter()
        .filter_map(|(i, s)| {
            if matches!(s.text_direction, TextDirection::TopToBottom) {
                None
            } else {
                Some(*i)
            }
        })
        .collect();

    if horizontal_idx.len() <= 1 {
        return indexed.into_iter().map(|(_, s)| s).collect();
    }

    // Sort by top-edge ascending (y-down normalised coords → top of page = 0).
    let mut order: Vec<usize> = horizontal_idx.clone();
    order.sort_by(|&a, &b| {
        let sa = &indexed[a].1;
        let sb = &indexed[b].1;
        sa.bbox
            .y
            .partial_cmp(&sb.bbox.y)
            .unwrap_or(Ordering::Equal)
            .then_with(|| sa.bbox.x.partial_cmp(&sb.bbox.x).unwrap_or(Ordering::Equal))
    });

    // Pairwise clustering on span-tops. Tolerance = 30% of the larger span's
    // height (≈ font-size for tight typography). No elastic range.
    let mut lines: Vec<Vec<usize>> = Vec::new();
    let mut cur: Vec<usize> = Vec::new();
    let mut prev_y: f32 = 0.0;
    let mut prev_h: f32 = 0.0;

    for idx in order {
        let s = &indexed[idx].1;
        let y = s.bbox.y;
        let h = s.bbox.height.max(1e-4);

        let same_line = if cur.is_empty() {
            true
        } else {
            let ref_h = h.max(prev_h);
            let tol = ref_h * LINE_CLUSTER_TOLERANCE;
            (y - prev_y).abs() <= tol
        };

        if same_line {
            cur.push(idx);
        } else {
            lines.push(std::mem::take(&mut cur));
            cur.push(idx);
        }
        prev_y = y;
        prev_h = h;
    }
    if !cur.is_empty() {
        lines.push(cur);
    }

    // Within each line, sort horizontal spans by x ascending (LTR).
    // (RTL spans carry their own reversed content already.)
    for line in &mut lines {
        line.sort_by(|&a, &b| {
            indexed[a]
                .1
                .bbox
                .x
                .partial_cmp(&indexed[b].1.bbox.x)
                .unwrap_or(Ordering::Equal)
        });
    }

    // Rebuild the full sequence: horizontal spans in the new line-clustered
    // order, with each TopToBottom span re-inserted at its original slot so
    // we don't disturb CJK vertical columns.
    let new_horizontal_order: Vec<usize> = lines.into_iter().flatten().collect();
    let mut new_horizontal_iter = new_horizontal_order.into_iter();

    let mut result: Vec<TextSpan> = Vec::with_capacity(indexed.len());
    let take_slots: Vec<bool> = indexed
        .iter()
        .map(|(_, s)| !matches!(s.text_direction, TextDirection::TopToBottom))
        .collect();

    // Extract spans by index (drain without cloning).
    let mut slots: Vec<Option<TextSpan>> = indexed.into_iter().map(|(_, s)| Some(s)).collect();

    for (i, is_horiz) in take_slots.iter().enumerate() {
        if *is_horiz {
            let next_idx = new_horizontal_iter
                .next()
                .expect("horizontal slot count mismatch");
            let span = slots[next_idx]
                .take()
                .expect("span already consumed during reorder");
            result.push(span);
        } else {
            let span = slots[i].take().expect("vertical span missing");
            result.push(span);
        }
    }

    result
}
