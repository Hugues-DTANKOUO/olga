use super::*;
use crate::model::{BoundingBox, Primitive, PrimitiveKind, TextDirection, ValueOrigin};

fn make_text_prim(x: f32, y: f32, width: f32, height: f32, text: &str) -> Primitive {
    Primitive::observed(
        PrimitiveKind::Text {
            content: text.to_string(),
            font_size: 12.0,
            font_name: "Helvetica".to_string(),
            is_bold: false,
            is_italic: false,
            color: None,
            char_positions: None,
            text_direction: TextDirection::LeftToRight,
            text_direction_origin: ValueOrigin::Observed,
            lang: None,
            lang_origin: None,
        },
        BoundingBox::new(x, y, width, height),
        0,
        0,
    )
}

#[test]
fn same_line_grouped_together() {
    let prims = vec![
        make_text_prim(0.05, 0.10, 0.20, 0.02, "Software Engineer"),
        make_text_prim(0.70, 0.10, 0.15, 0.02, "2022-2024"),
    ];
    let metrics = PageMetrics::from_primitives(&prims);
    let lines = group_visual_lines(&prims, &metrics);

    assert_eq!(lines.len(), 1, "Both spans should be on the same line");
    assert_eq!(lines[0].spans.len(), 2);
    assert_eq!(lines[0].spans[0].text, "Software Engineer");
    assert_eq!(lines[0].spans[1].text, "2022-2024");
}

#[test]
fn different_lines_separated() {
    let prims = vec![
        make_text_prim(0.05, 0.10, 0.20, 0.02, "Line 1"),
        make_text_prim(0.05, 0.15, 0.20, 0.02, "Line 2"),
    ];
    let metrics = PageMetrics::from_primitives(&prims);
    let lines = group_visual_lines(&prims, &metrics);

    assert_eq!(lines.len(), 2, "Should be two separate lines");
}

#[test]
fn proportional_spacing_preserved() {
    let prims = vec![
        make_text_prim(0.05, 0.10, 0.20, 0.02, "Title"),
        make_text_prim(0.80, 0.10, 0.10, 0.02, "Date"),
    ];
    let metrics = PageMetrics::from_primitives(&prims);
    let lines = group_visual_lines(&prims, &metrics);

    assert_eq!(lines.len(), 1);
    let rendered = render_line(&lines[0]);

    assert!(rendered.contains("Title"));
    assert!(rendered.contains("Date"));
    let title_end = rendered.find("Title").unwrap() + "Title".len();
    let date_start = rendered.find("Date").unwrap();
    assert!(
        date_start - title_end >= 5,
        "Expected spacing between title and date, got {} chars",
        date_start - title_end
    );
}

#[test]
fn render_line_output() {
    let line = VisualLine {
        spans: vec![
            AlignedSpan {
                col_start: 0,
                text: "Hello".to_string(),
                prim_index: 0,
                bbox: BoundingBox::new(0.0, 0.0, 0.1, 0.02),
            },
            AlignedSpan {
                col_start: 20,
                text: "World".to_string(),
                prim_index: 1,
                bbox: BoundingBox::new(0.5, 0.0, 0.1, 0.02),
            },
        ],
        bbox: BoundingBox::new(0.0, 0.0, 0.6, 0.02),
        prim_indices: vec![0, 1],
    };

    let rendered = render_line(&line);
    assert_eq!(&rendered[..5], "Hello");
    assert_eq!(&rendered[20..25], "World");
    assert_eq!(rendered.len(), 25);
}

#[test]
fn y_overlap_ratio_basic() {
    let a = BoundingBox::new(0.0, 0.10, 0.5, 0.02);
    let b = BoundingBox::new(0.5, 0.10, 0.3, 0.02);
    assert!(
        (super::barriers::y_overlap_ratio(&a, &b) - 1.0).abs() < 0.01,
        "Same Y → 100% overlap"
    );

    let c = BoundingBox::new(0.0, 0.10, 0.5, 0.02);
    let d = BoundingBox::new(0.5, 0.20, 0.3, 0.02);
    assert!(
        (super::barriers::y_overlap_ratio(&c, &d)).abs() < 0.01,
        "No Y overlap → 0%"
    );
}

#[test]
fn table_detection_conservative() {
    let prims: Vec<Primitive> = (0..4)
        .flat_map(|row| {
            let y = 0.10 + row as f32 * 0.03;
            vec![
                make_text_prim(0.05, y, 0.20, 0.02, "Col1"),
                make_text_prim(0.35, y, 0.20, 0.02, "Col2"),
                make_text_prim(0.65, y, 0.20, 0.02, "Col3"),
            ]
        })
        .collect();

    let metrics = PageMetrics::from_primitives(&prims);
    let lines = group_visual_lines(&prims, &metrics);
    assert!(looks_like_table(&lines, 3, 0.02));
}

#[test]
fn cv_layout_not_table() {
    let prims = vec![
        make_text_prim(0.05, 0.10, 0.30, 0.025, "Experience"),
        make_text_prim(0.05, 0.14, 0.25, 0.02, "Software Engineer"),
        make_text_prim(0.70, 0.14, 0.15, 0.02, "2022-2024"),
        make_text_prim(0.05, 0.17, 0.20, 0.02, "Acme Corp"),
        make_text_prim(0.08, 0.20, 0.50, 0.02, "- Built REST APIs"),
        make_text_prim(0.08, 0.23, 0.50, 0.02, "- Led team of 5"),
    ];

    let metrics = PageMetrics::from_primitives(&prims);
    let lines = group_visual_lines(&prims, &metrics);
    assert!(
        !looks_like_table(&lines, 3, 0.02),
        "CV layout should not be detected as a table"
    );
}
