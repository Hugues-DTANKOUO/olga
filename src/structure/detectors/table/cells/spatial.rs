use std::collections::HashSet;

use super::super::edges::{Edge, Orientation};
use super::super::intersections::Intersection;

pub(super) fn build_intersection_index(
    intersections: &[Intersection],
    xs: &[f32],
    ys: &[f32],
    tolerance: f32,
) -> HashSet<(usize, usize)> {
    let mut set = HashSet::with_capacity(intersections.len());
    for p in intersections {
        let yi = find_position_index(ys, p.y, tolerance);
        let xi = find_position_index(xs, p.x, tolerance);
        if let (Some(yi), Some(xi)) = (yi, xi) {
            set.insert((yi, xi));
        }
    }
    set
}

pub(super) fn build_edge_spans(
    positions: &[f32],
    edges: &[Edge],
    orientation: Orientation,
    tolerance: f32,
) -> Vec<Vec<(f32, f32)>> {
    let mut spans = vec![Vec::new(); positions.len()];

    for e in edges {
        if e.orientation != orientation {
            continue;
        }
        if let Some(idx) = find_position_index(positions, e.position, tolerance) {
            spans[idx].push((e.start, e.end));
        }
    }

    for span_list in &mut spans {
        if span_list.len() <= 1 {
            continue;
        }
        span_list.sort_by(|a, b| a.0.total_cmp(&b.0));

        let mut merged = vec![span_list[0]];
        for &(s, e) in &span_list[1..] {
            let last_idx = merged.len() - 1;
            if s <= merged[last_idx].1 + tolerance {
                merged[last_idx].1 = merged[last_idx].1.max(e);
            } else {
                merged.push((s, e));
            }
        }
        *span_list = merged;
    }

    spans
}

pub(super) fn find_position_index(positions: &[f32], value: f32, tolerance: f32) -> Option<usize> {
    let idx = positions.partition_point(|&p| p < value - tolerance);
    if idx < positions.len() && (positions[idx] - value).abs() <= tolerance {
        Some(idx)
    } else {
        None
    }
}

pub(super) fn spans_cover(spans: &[(f32, f32)], from: f32, to: f32, tolerance: f32) -> bool {
    let lo = from.min(to);
    let hi = from.max(to);
    spans
        .iter()
        .any(|(s, e)| *s <= lo + tolerance && *e >= hi - tolerance)
}

pub(super) fn unique_positions(iter: impl Iterator<Item = f32>, tolerance: f32) -> Vec<f32> {
    let mut positions: Vec<f32> = iter.collect();
    positions.sort_by(|a, b| a.total_cmp(b));

    if positions.is_empty() {
        return positions;
    }

    let mut unique = vec![positions[0]];
    let mut cluster_sum = positions[0];
    let mut cluster_count = 1u32;

    for &pos in &positions[1..] {
        let cluster_avg = cluster_sum / cluster_count as f32;
        if (pos - cluster_avg).abs() <= tolerance {
            cluster_sum += pos;
            cluster_count += 1;
            let last_idx = unique.len() - 1;
            unique[last_idx] = cluster_sum / cluster_count as f32;
        } else {
            unique.push(pos);
            cluster_sum = pos;
            cluster_count = 1;
        }
    }

    unique
}
