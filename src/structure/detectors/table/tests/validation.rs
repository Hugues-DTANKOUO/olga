use super::super::stream::{TextItem, cluster_x_positions};
use super::*;

#[test]
fn validate_clamps_invalid_confidence() {
    let mut d = TableDetector {
        lattice_confidence: 1.5,
        ..Default::default()
    };
    let issues = d.validate();
    assert!(!issues.is_empty());
    assert!((d.lattice_confidence - 0.85).abs() < f32::EPSILON);
}

#[test]
fn validate_clamps_invalid_min_columns() {
    let mut d = TableDetector {
        min_columns: 1,
        ..Default::default()
    };
    let issues = d.validate();
    assert!(!issues.is_empty());
    assert_eq!(d.min_columns, 2);
}

// -- Cluster helper tests -------------------------------------------------

#[test]
fn cluster_helper_three_pass_produces_six_positions() {
    // Two columns of text, well-separated on X.
    // width=0.08 is realistic (median_width ~0.10, tolerance = 0.015).
    //
    // Three-pass clustering (left/center/right):
    //   Pass x0:     [0.05, 0.06] → centroid ~0.055  |  [0.40, 0.41] → ~0.405
    //   Pass center: [0.09, 0.10] → centroid ~0.095  |  [0.44, 0.45] → ~0.445
    //   Pass x1:     [0.13, 0.14] → centroid ~0.135  |  [0.48, 0.49] → ~0.485
    //
    // Merged + dedup (tolerance 0.015): all 6 centroids are > 0.015 apart.
    // → exactly 6 column edge positions.
    //
    // This is expected: a three-pass clustering scheme produces multiple
    // edge candidates per visual column. The downstream grid construction
    // and cell validation filter redundant edges.
    let items = [
        TextItem {
            x: 0.05,
            y: 0.0,
            width: 0.08,
        },
        TextItem {
            x: 0.06,
            y: 0.0,
            width: 0.08,
        },
        TextItem {
            x: 0.40,
            y: 0.0,
            width: 0.08,
        },
        TextItem {
            x: 0.41,
            y: 0.0,
            width: 0.08,
        },
    ];
    let clusters = cluster_x_positions(&items, 0.015);
    assert_eq!(
        clusters.len(),
        6,
        "3 passes × 2 column groups = 6 edge positions"
    );
}
#[test]
fn validate_clamps_intersection_tolerance() {
    let mut d = TableDetector {
        intersection_tolerance: -0.01,
        ..Default::default()
    };
    let issues = d.validate();
    assert!(!issues.is_empty());
    assert!((d.intersection_tolerance - 0.005).abs() < f32::EPSILON);
}

#[test]
fn validate_clamps_x_tolerance() {
    let mut d = TableDetector {
        x_tolerance: 0.5,
        ..Default::default()
    };
    let issues = d.validate();
    assert!(!issues.is_empty());
    assert!((d.x_tolerance - 0.015).abs() < f32::EPSILON);
}

#[test]
fn validate_clamps_y_gap_factor() {
    let mut d = TableDetector {
        y_gap_factor: 0.5,
        ..Default::default()
    };
    let issues = d.validate();
    assert!(!issues.is_empty());
    assert!((d.y_gap_factor - 2.0).abs() < f32::EPSILON);
}

#[test]
fn validate_clamps_edge_config_fields() {
    let mut d = TableDetector::default();
    d.edge_config.snap_tolerance = -1.0;
    d.edge_config.join_tolerance = 0.0;
    d.edge_config.min_edge_length = -0.5;
    d.edge_config.max_line_thickness = 0.0;
    let issues = d.validate();
    assert_eq!(issues.len(), 4);
    assert!((d.edge_config.snap_tolerance - 0.005).abs() < f32::EPSILON);
    assert!((d.edge_config.join_tolerance - 0.005).abs() < f32::EPSILON);
    assert!((d.edge_config.min_edge_length - 0.01).abs() < f32::EPSILON);
    assert!((d.edge_config.max_line_thickness - 0.008).abs() < f32::EPSILON);
}
