use super::*;
use crate::model::{BoundingBox, Point as GeoPoint, Primitive, PrimitiveKind};

fn make_line_prim(x0: f32, y0: f32, x1: f32, y1: f32, thickness: f32) -> Primitive {
    Primitive::observed(
        PrimitiveKind::Line {
            start: GeoPoint::new(x0, y0),
            end: GeoPoint::new(x1, y1),
            thickness,
            color: None,
        },
        BoundingBox::new(x0.min(x1), y0.min(y1), (x1 - x0).abs(), (y1 - y0).abs()),
        0,
        0,
    )
}

fn make_rect_prim(x: f32, y: f32, w: f32, h: f32) -> Primitive {
    Primitive::observed(
        PrimitiveKind::Rectangle {
            fill: None,
            stroke: None,
            stroke_thickness: Some(0.001),
        },
        BoundingBox::new(x, y, w, h),
        0,
        0,
    )
}

#[test]
fn extract_horizontal_line() {
    let prims = vec![make_line_prim(0.1, 0.5, 0.9, 0.5, 0.001)];
    let edges = extract_edges(&prims, &EdgeConfig::default());
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].orientation, Orientation::Horizontal);
    assert!((edges[0].position - 0.5).abs() < 1e-6);
    assert!((edges[0].start - 0.1).abs() < 1e-6);
    assert!((edges[0].end - 0.9).abs() < 1e-6);
}

#[test]
fn extract_vertical_line() {
    let prims = vec![make_line_prim(0.3, 0.1, 0.3, 0.8, 0.001)];
    let edges = extract_edges(&prims, &EdgeConfig::default());
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].orientation, Orientation::Vertical);
    assert!((edges[0].position - 0.3).abs() < 1e-6);
}

#[test]
fn extract_rectangle_produces_four_edges() {
    let prims = vec![make_rect_prim(0.1, 0.2, 0.5, 0.3)];
    let edges = extract_edges(&prims, &EdgeConfig::default());
    // 2 horizontal (top y=0.2, bottom y=0.5) + 2 vertical (left x=0.1, right x=0.6)
    assert_eq!(edges.len(), 4);
    let h_count = edges
        .iter()
        .filter(|e| e.orientation == Orientation::Horizontal)
        .count();
    let v_count = edges
        .iter()
        .filter(|e| e.orientation == Orientation::Vertical)
        .count();
    assert_eq!(h_count, 2);
    assert_eq!(v_count, 2);
}

#[test]
fn short_edges_filtered() {
    // Line shorter than min_edge_length (0.01).
    let prims = vec![make_line_prim(0.1, 0.5, 0.105, 0.5, 0.001)];
    let edges = extract_edges(&prims, &EdgeConfig::default());
    assert!(edges.is_empty(), "Short edge should be filtered");
}

#[test]
fn thick_lines_filtered() {
    let prims = vec![make_line_prim(0.1, 0.5, 0.9, 0.5, 0.02)];
    let edges = extract_edges(&prims, &EdgeConfig::default());
    assert!(edges.is_empty(), "Thick line should be filtered");
}

#[test]
fn snap_merges_nearby_horizontal_edges() {
    let edges = vec![
        Edge {
            orientation: Orientation::Horizontal,
            position: 0.500,
            start: 0.1,
            end: 0.5,
        },
        Edge {
            orientation: Orientation::Horizontal,
            position: 0.503,
            start: 0.4,
            end: 0.9,
        },
    ];
    let config = EdgeConfig::default();
    let merged = snap_and_merge(edges, &config);

    let h: Vec<&Edge> = merged
        .iter()
        .filter(|e| e.orientation == Orientation::Horizontal)
        .collect();
    // Two edges at y≈0.500 and y≈0.503 should snap together,
    // and their overlapping segments [0.1,0.5] ∪ [0.4,0.9] → [0.1,0.9].
    assert_eq!(h.len(), 1, "Should merge into one edge");
    assert!((h[0].start - 0.1).abs() < 1e-3);
    assert!((h[0].end - 0.9).abs() < 1e-3);
}

#[test]
fn non_overlapping_edges_not_merged() {
    let edges = vec![
        Edge {
            orientation: Orientation::Horizontal,
            position: 0.5,
            start: 0.1,
            end: 0.3,
        },
        Edge {
            orientation: Orientation::Horizontal,
            position: 0.5,
            start: 0.6,
            end: 0.9,
        },
    ];
    let config = EdgeConfig::default();
    let merged = snap_and_merge(edges, &config);
    let h: Vec<&Edge> = merged
        .iter()
        .filter(|e| e.orientation == Orientation::Horizontal)
        .collect();
    assert_eq!(h.len(), 2, "Non-overlapping edges should stay separate");
}

#[test]
fn distant_edges_not_snapped() {
    let edges = vec![
        Edge {
            orientation: Orientation::Horizontal,
            position: 0.2,
            start: 0.1,
            end: 0.9,
        },
        Edge {
            orientation: Orientation::Horizontal,
            position: 0.8,
            start: 0.1,
            end: 0.9,
        },
    ];
    let config = EdgeConfig::default();
    let merged = snap_and_merge(edges, &config);
    assert_eq!(merged.len(), 2, "Distant edges should not snap");
}
