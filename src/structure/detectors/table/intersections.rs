//! Intersection detection — Step 3 of the table detection pipeline.
//!
//! For every pair (horizontal edge, vertical edge), check whether they cross
//! within tolerance. Each valid crossing is a potential table-cell corner.

use std::collections::HashMap;

use super::edges::{Edge, Orientation};

// ---------------------------------------------------------------------------
// Intersection point
// ---------------------------------------------------------------------------

/// A point where a horizontal edge and a vertical edge cross.
#[derive(Debug, Clone, Copy)]
pub struct Intersection {
    pub x: f32,
    pub y: f32,
}

// ---------------------------------------------------------------------------
// Step 3: Find intersections
// ---------------------------------------------------------------------------

/// Find all points where a horizontal edge crosses a vertical edge.
///
/// An intersection exists when:
/// - The vertical edge's X is within the horizontal edge's X range (± tolerance).
/// - The horizontal edge's Y is within the vertical edge's Y range (± tolerance).
///
/// Optimized: vertical edges are sorted by X position so that each horizontal
/// edge only checks the subset of verticals whose X falls within its range,
/// using binary search for O(H × log V + K) where K = result count.
pub fn find_intersections(edges: &[Edge], tolerance: f32) -> Vec<Intersection> {
    let h_edges: Vec<&Edge> = edges
        .iter()
        .filter(|e| e.orientation == Orientation::Horizontal)
        .collect();
    let mut v_edges: Vec<&Edge> = edges
        .iter()
        .filter(|e| e.orientation == Orientation::Vertical)
        .collect();

    // Sort vertical edges by X position for binary-search narrowing.
    v_edges.sort_by(|a, b| a.position.total_cmp(&b.position));

    let mut intersections = Vec::new();

    for h in &h_edges {
        let h_y = h.position;
        let x_lo = h.start - tolerance;
        let x_hi = h.end + tolerance;

        // Binary search: find first V edge with position >= x_lo.
        let start_idx = v_edges.partition_point(|v| v.position < x_lo);

        // Scan only V edges in [x_lo, x_hi].
        for v in &v_edges[start_idx..] {
            if v.position > x_hi {
                break; // All remaining V edges have larger X — done.
            }
            // Check: horizontal Y is within vertical's Y range.
            if h_y >= (v.start - tolerance) && h_y <= (v.end + tolerance) {
                intersections.push(Intersection {
                    x: v.position,
                    y: h_y,
                });
            }
        }
    }

    // Deduplicate intersections that are within tolerance of each other.
    deduplicate_intersections(&mut intersections, tolerance);

    intersections
}

/// Remove near-duplicate intersection points using spatial hashing (O(n)).
///
/// Points are placed into a grid of cells sized `tolerance`. Two points
/// within tolerance may land in the same or neighboring buckets, so each
/// point checks at most 9 neighbors. The first point in a bucket "wins";
/// subsequent points within tolerance are dropped.
fn deduplicate_intersections(points: &mut Vec<Intersection>, tolerance: f32) {
    if points.len() <= 1 {
        return;
    }

    // Sort for deterministic output order (Y then X).
    points.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));

    // Quantize a coordinate to a grid cell index.
    let quant = |v: f32| -> i64 { (v / tolerance).floor() as i64 };

    // Map from grid cell to the index of the representative point.
    let mut grid: HashMap<(i64, i64), usize> = HashMap::with_capacity(points.len());
    let mut keep = vec![false; points.len()];

    for (i, p) in points.iter().enumerate() {
        let qx = quant(p.x);
        let qy = quant(p.y);

        // Check if any existing representative in the 3×3 neighborhood
        // is within tolerance of this point.
        let mut is_duplicate = false;
        'outer: for dx in -1..=1_i64 {
            for dy in -1..=1_i64 {
                if let Some(&rep_idx) = grid.get(&(qx + dx, qy + dy)) {
                    let rep = &points[rep_idx];
                    if (p.x - rep.x).abs() <= tolerance && (p.y - rep.y).abs() <= tolerance {
                        is_duplicate = true;
                        break 'outer;
                    }
                }
            }
        }

        if !is_duplicate {
            keep[i] = true;
            grid.insert((qx, qy), i);
        }
    }

    let mut write_idx = 0;
    for read_idx in 0..points.len() {
        if keep[read_idx] {
            points[write_idx] = points[read_idx];
            write_idx += 1;
        }
    }
    points.truncate(write_idx);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::super::edges::{Edge, Orientation};
    use super::*;

    fn h_edge(y: f32, x_start: f32, x_end: f32) -> Edge {
        Edge {
            orientation: Orientation::Horizontal,
            position: y,
            start: x_start,
            end: x_end,
        }
    }

    fn v_edge(x: f32, y_start: f32, y_end: f32) -> Edge {
        Edge {
            orientation: Orientation::Vertical,
            position: x,
            start: y_start,
            end: y_end,
        }
    }

    #[test]
    fn simple_cross() {
        // One H edge at y=0.5, one V edge at x=0.3.
        let edges = vec![h_edge(0.5, 0.1, 0.9), v_edge(0.3, 0.2, 0.8)];
        let ints = find_intersections(&edges, 0.005);
        assert_eq!(ints.len(), 1);
        assert!((ints[0].x - 0.3).abs() < 1e-6);
        assert!((ints[0].y - 0.5).abs() < 1e-6);
    }

    #[test]
    fn no_intersection_when_parallel() {
        let edges = vec![h_edge(0.3, 0.1, 0.9), h_edge(0.7, 0.1, 0.9)];
        let ints = find_intersections(&edges, 0.005);
        assert!(ints.is_empty());
    }

    #[test]
    fn no_intersection_when_not_overlapping() {
        // H edge at y=0.5, x=[0.1, 0.4]. V edge at x=0.6, y=[0.2, 0.8].
        // They don't overlap in X range.
        let edges = vec![h_edge(0.5, 0.1, 0.4), v_edge(0.6, 0.2, 0.8)];
        let ints = find_intersections(&edges, 0.005);
        assert!(ints.is_empty());
    }

    #[test]
    fn grid_intersections() {
        // 3×3 grid: 3 H edges × 3 V edges → 9 intersections.
        let edges = vec![
            h_edge(0.2, 0.1, 0.9),
            h_edge(0.5, 0.1, 0.9),
            h_edge(0.8, 0.1, 0.9),
            v_edge(0.1, 0.1, 0.9),
            v_edge(0.5, 0.1, 0.9),
            v_edge(0.9, 0.1, 0.9),
        ];
        let ints = find_intersections(&edges, 0.005);
        assert_eq!(ints.len(), 9);
    }

    #[test]
    fn near_duplicate_intersections_deduped() {
        // Two very close intersections from slightly offset edges.
        let edges = vec![
            h_edge(0.500, 0.1, 0.9),
            h_edge(0.503, 0.1, 0.9),
            v_edge(0.3, 0.2, 0.8),
        ];
        let ints = find_intersections(&edges, 0.005);
        // Both h-edges cross the v-edge at x=0.3, y≈0.500 and y≈0.503.
        // These are within tolerance, so they should dedup to 1.
        assert_eq!(ints.len(), 1);
    }

    #[test]
    fn intersection_at_edge_endpoint_within_tolerance() {
        // V edge ends at y=0.5, H edge is at y=0.503 (within tolerance).
        let edges = vec![h_edge(0.503, 0.1, 0.9), v_edge(0.3, 0.2, 0.5)];
        let ints = find_intersections(&edges, 0.005);
        assert_eq!(
            ints.len(),
            1,
            "Should detect intersection at endpoint within tolerance"
        );
    }

    #[test]
    fn dedup_spatial_hash_removes_cluster() {
        // Multiple very close intersections from a dense grid should dedup.
        let edges = vec![
            h_edge(0.500, 0.1, 0.9),
            h_edge(0.501, 0.1, 0.9),
            h_edge(0.502, 0.1, 0.9),
            v_edge(0.3, 0.2, 0.8),
        ];
        let ints = find_intersections(&edges, 0.005);
        // All 3 H-edges cross V-edge at x=0.3, producing 3 points within
        // tolerance — should dedup to 1.
        assert_eq!(ints.len(), 1, "Spatial hash should dedup clustered points");
    }

    #[test]
    fn dedup_preserves_distant_points() {
        // Two well-separated intersections should both survive dedup.
        let edges = vec![
            h_edge(0.2, 0.1, 0.9),
            h_edge(0.8, 0.1, 0.9),
            v_edge(0.5, 0.1, 0.9),
        ];
        let ints = find_intersections(&edges, 0.005);
        assert_eq!(ints.len(), 2, "Distant points should not be deduped");
    }
}
