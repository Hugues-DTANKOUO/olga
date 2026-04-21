//! Step 2 of the edges pipeline — snap nearby parallel edges to the same
//! position and merge overlapping collinear segments.
//!
//! Works orientation by orientation (horizontals and verticals never
//! interact). Within one orientation: cluster by position within
//! `snap_tolerance`, collapse each cluster to its average position, then
//! merge segments whose endpoints touch or overlap within `join_tolerance`.

use super::types::{Edge, EdgeConfig, Orientation};

/// Snap nearby parallel edges to the same position, then merge overlapping
/// collinear segments (Step 2).
///
/// 1. Separate edges by orientation.
/// 2. Sort by position (Y for horizontal, X for vertical).
/// 3. Cluster edges within `snap_tolerance` of each other → average position.
/// 4. Within each cluster, merge overlapping/touching segments.
pub fn snap_and_merge(edges: Vec<Edge>, config: &EdgeConfig) -> Vec<Edge> {
    let mut h_edges: Vec<Edge> = edges
        .iter()
        .filter(|e| e.orientation == Orientation::Horizontal)
        .copied()
        .collect();
    let mut v_edges: Vec<Edge> = edges
        .iter()
        .filter(|e| e.orientation == Orientation::Vertical)
        .copied()
        .collect();

    h_edges = snap_and_merge_1d(h_edges, config.snap_tolerance, config.join_tolerance);
    v_edges = snap_and_merge_1d(v_edges, config.snap_tolerance, config.join_tolerance);

    h_edges.extend(v_edges);
    h_edges
}

/// Internal: snap and merge edges of the same orientation.
fn snap_and_merge_1d(mut edges: Vec<Edge>, snap_tolerance: f32, join_tolerance: f32) -> Vec<Edge> {
    if edges.is_empty() {
        return edges;
    }

    // Sort by position.
    edges.sort_by(|a, b| a.position.total_cmp(&b.position));

    // Cluster by position within snap_tolerance.
    let mut clusters: Vec<Vec<Edge>> = vec![vec![edges[0]]];

    for &edge in &edges[1..] {
        // Safety: `clusters` always has at least one element (initialized above)
        // and elements are never removed.
        let last_idx = clusters.len() - 1;
        let last_cluster = &clusters[last_idx];
        // Use the average position of the cluster for comparison.
        let avg_pos =
            last_cluster.iter().map(|e| e.position).sum::<f32>() / last_cluster.len() as f32;

        if (edge.position - avg_pos).abs() <= snap_tolerance {
            clusters[last_idx].push(edge);
        } else {
            clusters.push(vec![edge]);
        }
    }

    // For each cluster: snap position to average, then merge overlapping segments.
    let mut result = Vec::new();

    for cluster in &clusters {
        let avg_pos = cluster.iter().map(|e| e.position).sum::<f32>() / cluster.len() as f32;
        let orientation = cluster[0].orientation;

        // Collect all segments, snapped to the average position.
        let mut segments: Vec<(f32, f32)> = cluster.iter().map(|e| (e.start, e.end)).collect();

        // Sort by start.
        segments.sort_by(|a, b| a.0.total_cmp(&b.0));

        // Merge overlapping/touching segments.
        let mut merged: Vec<(f32, f32)> = vec![segments[0]];

        for &(s, e) in &segments[1..] {
            // Safety: `merged` always has at least one element (initialized above).
            let last_idx = merged.len() - 1;
            if s <= merged[last_idx].1 + join_tolerance {
                // Overlapping or touching → extend.
                merged[last_idx].1 = merged[last_idx].1.max(e);
            } else {
                merged.push((s, e));
            }
        }

        for (s, e) in merged {
            result.push(Edge {
                orientation,
                position: avg_pos,
                start: s,
                end: e,
            });
        }
    }

    result
}
