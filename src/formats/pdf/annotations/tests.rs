use super::overlap::{bbox_overlap_sufficient, overlap_score};
use crate::model::geometry::BoundingBox;

#[test]
fn test_bbox_overlap_full() {
    let prim = BoundingBox {
        x: 0.1,
        y: 0.2,
        width: 0.3,
        height: 0.02,
    };
    let annot = BoundingBox {
        x: 0.05,
        y: 0.19,
        width: 0.4,
        height: 0.04,
    };
    assert!(bbox_overlap_sufficient(&prim, &annot));
}

#[test]
fn test_bbox_overlap_insufficient_horizontal() {
    let prim = BoundingBox {
        x: 0.1,
        y: 0.2,
        width: 0.3,
        height: 0.02,
    };
    let annot = BoundingBox {
        x: 0.0,
        y: 0.19,
        width: 0.2,
        height: 0.04,
    };
    assert!(!bbox_overlap_sufficient(&prim, &annot));
}

#[test]
fn test_bbox_no_vertical_overlap() {
    let prim = BoundingBox {
        x: 0.1,
        y: 0.2,
        width: 0.3,
        height: 0.02,
    };
    let annot = BoundingBox {
        x: 0.05,
        y: 0.5,
        width: 0.4,
        height: 0.04,
    };
    assert!(!bbox_overlap_sufficient(&prim, &annot));
}

#[test]
fn overlap_score_prefers_better_center_match() {
    let prim = BoundingBox {
        x: 0.10,
        y: 0.20,
        width: 0.20,
        height: 0.02,
    };
    let better = BoundingBox {
        x: 0.09,
        y: 0.19,
        width: 0.22,
        height: 0.04,
    };
    let worse = BoundingBox {
        x: 0.00,
        y: 0.19,
        width: 0.30,
        height: 0.04,
    };
    let sb = overlap_score(&prim, &better).unwrap_or(0.0);
    let sw = overlap_score(&prim, &worse).unwrap_or(0.0);
    assert!(sb > sw);
}
