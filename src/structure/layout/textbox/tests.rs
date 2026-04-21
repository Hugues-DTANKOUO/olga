use super::grouping::{group_lines_greedy, group_lines_into_boxes_rtree};
use super::*;
use crate::model::BoundingBox;

fn create_test_line(x: f32, y: f32, width: f32, height: f32) -> TextLine {
    let span = crate::structure::layout::types::TextSpan {
        primitive_index: 0,
        bbox: BoundingBox {
            x,
            y,
            width,
            height,
        },
        font_size: height,
        is_bold: false,
        is_italic: false,
        text: "test".to_string(),
    };
    TextLine::from_spans(vec![span])
}

fn default_params() -> LAParams {
    LAParams {
        line_margin: 0.5,
        word_margin: 0.1,
        line_overlap_threshold: 0.5,
        boxes_flow: Some(0.5),
        column_gap_fraction: 0.1,
    }
}

#[test]
fn test_greedy_vs_rtree_equivalence() {
    // Small input: both paths should be taken and produce equivalent results.
    let lines = vec![
        create_test_line(0.0, 0.0, 1.0, 0.05),
        create_test_line(0.0, 0.06, 1.0, 0.05),
        create_test_line(0.0, 0.15, 1.0, 0.05),
    ];

    let params = default_params();

    let mut greedy_boxes = group_lines_greedy(&lines, &params);
    let mut rtree_boxes = group_lines_into_boxes_rtree(&lines, &params);

    // Sort both by Y of first line so order is deterministic.
    greedy_boxes.sort_by(|a, b| a.bbox.y.total_cmp(&b.bbox.y));
    rtree_boxes.sort_by(|a, b| a.bbox.y.total_cmp(&b.bbox.y));

    assert_eq!(
        greedy_boxes.len(),
        rtree_boxes.len(),
        "Greedy and R-tree should produce same number of boxes"
    );

    for (greedy_box, rtree_box) in greedy_boxes.iter().zip(rtree_boxes.iter()) {
        assert_eq!(
            greedy_box.lines.len(),
            rtree_box.lines.len(),
            "Greedy and R-tree boxes should have same number of lines"
        );
    }
}

#[test]
fn test_rtree_large_input() {
    // Create 100+ lines to ensure R-tree path is taken.
    let mut lines = Vec::new();
    let params = default_params();

    // Create 5 columns of 25 lines each, with vertical gaps within threshold.
    for col in 0..5 {
        let col_x = (col as f32) * 0.25;
        for row in 0..25 {
            let y = (row as f32) * 0.06; // 0.06 gap with median height 0.05
            lines.push(create_test_line(col_x, y, 0.2, 0.05));
        }
    }

    assert!(lines.len() >= 100);

    let boxes = group_lines_into_boxes(&lines, &params);

    // With horizontal grouping and vertical merging, we should have
    // a reasonable number of boxes (not 125 and not 1).
    assert!(boxes.len() > 1);
    assert!(boxes.len() < lines.len());

    // Each box should have at least one line.
    for box_item in &boxes {
        assert!(!box_item.lines.is_empty());
    }
}

#[test]
fn test_small_input_uses_greedy() {
    // With < 50 lines, dispatch should use greedy.
    let lines = vec![
        create_test_line(0.0, 0.0, 1.0, 0.05),
        create_test_line(0.0, 0.06, 1.0, 0.05),
    ];

    let params = default_params();
    let boxes = group_lines_into_boxes(&lines, &params);

    // These two lines should be in the same box (gap < threshold).
    assert_eq!(boxes.len(), 1);
    assert_eq!(boxes[0].lines.len(), 2);
}

#[test]
fn test_large_vertical_gap_separates_boxes() {
    // Lines with large vertical gap should be in separate boxes.
    let lines = vec![
        create_test_line(0.0, 0.0, 1.0, 0.05),
        create_test_line(0.0, 0.5, 1.0, 0.05), // Large gap
    ];

    let params = default_params();
    let boxes = group_lines_into_boxes(&lines, &params);

    assert_eq!(boxes.len(), 2);
    assert_eq!(boxes[0].lines.len(), 1);
    assert_eq!(boxes[1].lines.len(), 1);
}

#[test]
fn test_no_horizontal_overlap_prevents_merge() {
    // Two lines in different horizontal regions should not merge.
    let lines = vec![
        create_test_line(0.0, 0.0, 0.4, 0.05),
        create_test_line(0.7, 0.06, 0.4, 0.05), // No horizontal overlap
    ];

    let params = default_params();
    let boxes = group_lines_into_boxes(&lines, &params);

    // Should be in different clusters due to lack of horizontal overlap.
    assert_eq!(boxes.len(), 2);
}
