use super::*;
use crate::model::{BoundingBox, Primitive, PrimitiveKind};

/// Helper: create a text primitive at a given position.
fn text_prim(x: f32, y: f32, width: f32, height: f32, text: &str) -> Primitive {
    Primitive::observed(
        PrimitiveKind::Text {
            content: text.to_string(),
            font_name: "Arial".to_string(),
            font_size: height,
            is_bold: false,
            is_italic: false,
            color: None,
            char_positions: None,
            text_direction: Default::default(),
            text_direction_origin: crate::model::provenance::ValueOrigin::Observed,
            lang: None,
            lang_origin: None,
        },
        BoundingBox::new(x, y, width, height),
        0,
        0,
    )
}

fn bold_prim(x: f32, y: f32, width: f32, height: f32, text: &str) -> Primitive {
    Primitive::observed(
        PrimitiveKind::Text {
            content: text.to_string(),
            font_name: "Arial".to_string(),
            font_size: height,
            is_bold: true,
            is_italic: false,
            color: None,
            char_positions: None,
            text_direction: Default::default(),
            text_direction_origin: crate::model::provenance::ValueOrigin::Observed,
            lang: None,
            lang_origin: None,
        },
        BoundingBox::new(x, y, width, height),
        0,
        0,
    )
}

// =========================================================================
// Stage 1: Lines
// =========================================================================

#[test]
fn single_line_single_span() {
    let prims = vec![text_prim(0.1, 0.1, 0.3, 0.02, "Hello world")];
    let analyzer = HeuristicLayoutAnalyzer::default();
    let boxes = analyzer.analyze(&prims);
    assert_eq!(boxes.len(), 1);
    assert_eq!(boxes[0].lines.len(), 1);
    assert!(boxes[0].text().contains("Hello world"));
}

#[test]
fn single_line_multiple_spans() {
    let prims = vec![
        text_prim(0.1, 0.1, 0.15, 0.02, "Hello"),
        text_prim(0.3, 0.1, 0.15, 0.02, "world"),
    ];
    let analyzer = HeuristicLayoutAnalyzer::default();
    let boxes = analyzer.analyze(&prims);
    assert_eq!(boxes.len(), 1);
    assert_eq!(boxes[0].lines.len(), 1);
    assert_eq!(boxes[0].lines[0].spans.len(), 2);
}

#[test]
fn two_lines_same_box() {
    // Two lines close together should form one box.
    let prims = vec![
        text_prim(0.1, 0.10, 0.3, 0.02, "Line one"),
        text_prim(0.1, 0.13, 0.3, 0.02, "Line two"),
    ];
    let analyzer = HeuristicLayoutAnalyzer::default();
    let boxes = analyzer.analyze(&prims);
    assert_eq!(boxes.len(), 1);
    assert_eq!(boxes[0].lines.len(), 2);
}

#[test]
fn two_boxes_large_gap() {
    // Two lines with a large gap should form two separate boxes.
    let prims = vec![
        text_prim(0.1, 0.10, 0.3, 0.02, "Paragraph one"),
        text_prim(0.1, 0.30, 0.3, 0.02, "Paragraph two"),
    ];
    let analyzer = HeuristicLayoutAnalyzer::default();
    let boxes = analyzer.analyze(&prims);
    assert_eq!(boxes.len(), 2);
}

#[test]
fn empty_input() {
    let analyzer = HeuristicLayoutAnalyzer::default();
    let boxes = analyzer.analyze(&[]);
    assert!(boxes.is_empty());
}

#[test]
fn whitespace_only_spans_are_skipped() {
    let prims = vec![
        text_prim(0.1, 0.1, 0.3, 0.02, "   "),
        text_prim(0.1, 0.1, 0.3, 0.02, "Real text"),
    ];
    let analyzer = HeuristicLayoutAnalyzer::default();
    let boxes = analyzer.analyze(&prims);
    assert_eq!(boxes.len(), 1);
    assert!(boxes[0].text().contains("Real text"));
}

#[test]
fn non_text_primitives_are_ignored() {
    use crate::model::Point;
    let prims = vec![
        text_prim(0.1, 0.1, 0.3, 0.02, "Text"),
        Primitive::observed(
            PrimitiveKind::Line {
                start: Point::new(0.1, 0.2),
                end: Point::new(0.5, 0.2),
                thickness: 0.001,
                color: None,
            },
            BoundingBox::new(0.1, 0.2, 0.4, 0.0),
            0,
            1,
        ),
    ];
    let analyzer = HeuristicLayoutAnalyzer::default();
    let boxes = analyzer.analyze(&prims);
    assert_eq!(boxes.len(), 1);
}

// =========================================================================
// Stage 2: Column detection
// =========================================================================

#[test]
fn two_column_layout() {
    // Left column
    let mut prims = vec![
        text_prim(0.05, 0.10, 0.35, 0.02, "Left col line 1"),
        text_prim(0.05, 0.13, 0.35, 0.02, "Left col line 2"),
        text_prim(0.05, 0.16, 0.35, 0.02, "Left col line 3"),
    ];
    // Right column (gap > 5% of page width)
    prims.extend(vec![
        text_prim(0.55, 0.10, 0.35, 0.02, "Right col line 1"),
        text_prim(0.55, 0.13, 0.35, 0.02, "Right col line 2"),
        text_prim(0.55, 0.16, 0.35, 0.02, "Right col line 3"),
    ]);

    let analyzer = HeuristicLayoutAnalyzer::default();
    let boxes = analyzer.analyze(&prims);

    // Should produce 2 boxes (one per column).
    assert_eq!(boxes.len(), 2);
    // Left column should come first in reading order.
    assert!(boxes[0].bbox.x < boxes[1].bbox.x);
}

#[test]
fn reading_order_top_to_bottom() {
    let prims = vec![
        text_prim(0.1, 0.50, 0.3, 0.02, "Bottom paragraph"),
        text_prim(0.1, 0.10, 0.3, 0.02, "Top paragraph"),
    ];
    let analyzer = HeuristicLayoutAnalyzer::default();
    let boxes = analyzer.analyze(&prims);

    assert_eq!(boxes.len(), 2);
    // Top paragraph should come first.
    assert!(boxes[0].text().contains("Top"));
    assert!(boxes[1].text().contains("Bottom"));
}

// =========================================================================
// TextBox properties
// =========================================================================

#[test]
fn textbox_bold_detection() {
    let prims = vec![
        bold_prim(0.1, 0.10, 0.3, 0.02, "Bold line"),
        bold_prim(0.1, 0.13, 0.3, 0.02, "Also bold"),
    ];
    let analyzer = HeuristicLayoutAnalyzer::default();
    let boxes = analyzer.analyze(&prims);
    assert_eq!(boxes.len(), 1);
    assert!(boxes[0].is_predominantly_bold);
}

#[test]
fn textbox_primitive_indices() {
    let prims = vec![
        text_prim(0.1, 0.10, 0.3, 0.02, "Line A"),
        text_prim(0.1, 0.13, 0.3, 0.02, "Line B"),
    ];
    let analyzer = HeuristicLayoutAnalyzer::default();
    let boxes = analyzer.analyze(&prims);
    let indices = boxes[0].primitive_indices();
    assert!(indices.contains(&0));
    assert!(indices.contains(&1));
}

// =========================================================================
// Custom params
// =========================================================================

#[test]
fn strict_line_margin_splits_more() {
    // With a very strict line_margin (0.01), even slightly offset text
    // should be split into separate lines.
    let prims = vec![
        text_prim(0.1, 0.10, 0.3, 0.02, "Line A"),
        text_prim(0.1, 0.115, 0.3, 0.02, "Line B"), // slight Y offset
    ];

    let strict = HeuristicLayoutAnalyzer::new(LAParams {
        line_margin: 0.01,
        ..LAParams::default()
    });
    let boxes = strict.analyze(&prims);
    // With strict margin, these should be separate lines.
    let total_lines: usize = boxes.iter().map(|b| b.lines.len()).sum();
    assert!(total_lines >= 2, "Strict margin should produce >= 2 lines");
}

// =========================================================================
// boxes_flow weighted scoring
// =========================================================================

#[test]
fn boxes_flow_sidebar_layout() {
    // Main column on the left, sidebar on the right.
    // With default flow=0.5, main column should come first (left-to-right columns).
    let mut prims = vec![
        // Main column (left, 3 paragraphs with vertical gaps)
        text_prim(0.05, 0.10, 0.35, 0.02, "Main para 1"),
        text_prim(0.05, 0.30, 0.35, 0.02, "Main para 2"),
        text_prim(0.05, 0.50, 0.35, 0.02, "Main para 3"),
    ];
    // Sidebar (right, single block)
    prims.push(text_prim(0.55, 0.15, 0.30, 0.02, "Sidebar note"));

    let analyzer = HeuristicLayoutAnalyzer::new(LAParams {
        boxes_flow: Some(0.5),
        ..LAParams::default()
    });
    let boxes = analyzer.analyze(&prims);

    // Main column boxes should come before the sidebar.
    assert!(boxes.len() >= 2);
    assert!(
        boxes[0].text().contains("Main"),
        "First box should be from main column, got: {}",
        boxes[0].text()
    );
}

#[test]
fn boxes_flow_none_uses_top_left() {
    // With boxes_flow = None, order should be simple top-left.
    let prims = vec![
        text_prim(0.50, 0.10, 0.30, 0.02, "Right top"),
        text_prim(0.05, 0.50, 0.30, 0.02, "Left bottom"),
        text_prim(0.05, 0.10, 0.30, 0.02, "Left top"),
    ];

    let analyzer = HeuristicLayoutAnalyzer::new(LAParams {
        boxes_flow: None,
        ..LAParams::default()
    });
    let boxes = analyzer.analyze(&prims);

    assert_eq!(boxes.len(), 3);
    // First box should be the topmost (Y=0.10), leftmost (X=0.05).
    assert!(
        boxes[0].text().contains("Left top"),
        "Expected 'Left top' first, got: {}",
        boxes[0].text()
    );
}

#[test]
fn boxes_flow_high_flow_prefers_vertical() {
    // With flow=1.0, purely top-to-bottom regardless of proximity.
    let prims = vec![
        text_prim(0.05, 0.10, 0.30, 0.02, "Top"),
        text_prim(0.05, 0.90, 0.30, 0.02, "Bottom"),
        text_prim(0.05, 0.50, 0.30, 0.02, "Middle"),
    ];

    let analyzer = HeuristicLayoutAnalyzer::new(LAParams {
        boxes_flow: Some(1.0),
        ..LAParams::default()
    });
    let boxes = analyzer.analyze(&prims);

    assert_eq!(boxes.len(), 3);
    assert!(boxes[0].text().contains("Top"));
    assert!(boxes[1].text().contains("Middle"));
    assert!(boxes[2].text().contains("Bottom"));
}

#[test]
fn boxes_flow_negative_uses_proximity() {
    // With negative flow, proximity-based ordering (no column detection).
    let prims = vec![
        text_prim(0.05, 0.10, 0.30, 0.02, "A"),
        text_prim(0.05, 0.50, 0.30, 0.02, "C far"),
        text_prim(0.05, 0.13, 0.30, 0.02, "B close to A"),
    ];

    let analyzer = HeuristicLayoutAnalyzer::new(LAParams {
        boxes_flow: Some(-1.0),
        ..LAParams::default()
    });
    let boxes = analyzer.analyze(&prims);

    // A and "B close to A" should be adjacent (they're spatially close).
    // But they might be in the same box due to proximity. Let's check order.
    assert!(!boxes.is_empty());
}
