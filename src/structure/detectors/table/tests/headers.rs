use super::*;

fn bold_text_at(x: f32, y: f32, content: &str, order: u64) -> Primitive {
    Primitive::observed(
        PrimitiveKind::Text {
            content: content.to_string(),
            font_size: 12.0,
            font_name: "Arial".to_string(),
            is_bold: true,
            is_italic: false,
            color: None,
            char_positions: None,
            text_direction: Default::default(),
            text_direction_origin: crate::model::provenance::ValueOrigin::Observed,
            lang: None,
            lang_origin: None,
        },
        BoundingBox::new(x, y, 0.15, 0.02),
        0,
        order,
    )
}

fn large_text_at(x: f32, y: f32, content: &str, font_size: f32, order: u64) -> Primitive {
    Primitive::observed(
        PrimitiveKind::Text {
            content: content.to_string(),
            font_size,
            font_name: "Arial".to_string(),
            is_bold: false,
            is_italic: false,
            color: None,
            char_positions: None,
            text_direction: Default::default(),
            text_direction_origin: crate::model::provenance::ValueOrigin::Observed,
            lang: None,
            lang_origin: None,
        },
        BoundingBox::new(x, y, 0.15, 0.02),
        0,
        order,
    )
}

#[test]
fn bold_header_detected_lattice() {
    // 2×2 table: bold header row, normal body.
    let mut page = vec![
        bold_text_at(0.10, 0.12, "Name", 0),
        bold_text_at(0.30, 0.12, "Age", 1),
        text_at(0.10, 0.22, "Alice", 2),
        text_at(0.30, 0.22, "30", 3),
    ];
    // 3 horizontal + 3 vertical lines.
    page.push(h_line(0.10, 0.05, 0.50, 4));
    page.push(h_line(0.17, 0.05, 0.50, 5));
    page.push(h_line(0.27, 0.05, 0.50, 6));
    page.push(v_line(0.05, 0.10, 0.27, 7));
    page.push(v_line(0.25, 0.10, 0.27, 8));
    page.push(v_line(0.50, 0.10, 0.27, 9));

    let detector = TableDetector::default();
    let hints = detector.detect(&page, &ctx());

    assert_eq!(hints.len(), 4);

    // Row 0 should be TableHeader, row 1 should be TableCell.
    let header_hints: Vec<&DetectedHint> = hints
        .iter()
        .filter(|h| matches!(h.hint.kind, HintKind::TableHeader { .. }))
        .collect();
    assert_eq!(header_hints.len(), 2, "Row 0 should be detected as header");

    let cell_hints: Vec<&DetectedHint> = hints
        .iter()
        .filter(|h| matches!(h.hint.kind, HintKind::TableCell { .. }))
        .collect();
    assert_eq!(cell_hints.len(), 2, "Row 1 should remain as TableCell");
}

#[test]
fn bold_header_detected_stream() {
    // Borderless table with bold header row.
    let page = vec![
        bold_text_at(0.05, 0.10, "Name", 0),
        bold_text_at(0.40, 0.10, "Age", 1),
        text_at(0.05, 0.15, "Alice", 2),
        text_at(0.40, 0.15, "30", 3),
        text_at(0.05, 0.20, "Bob", 4),
        text_at(0.40, 0.20, "25", 5),
    ];

    let detector = TableDetector::default();
    let hints = detector.detect(&page, &ctx());

    assert_eq!(hints.len(), 6);
    let headers: Vec<&DetectedHint> = hints
        .iter()
        .filter(|h| matches!(h.hint.kind, HintKind::TableHeader { .. }))
        .collect();
    assert_eq!(headers.len(), 2, "Bold row 0 → TableHeader in stream mode");
}

#[test]
fn larger_font_header_detected() {
    // Header row with larger font size.
    let mut page = vec![
        large_text_at(0.10, 0.12, "Name", 14.0, 0),
        large_text_at(0.30, 0.12, "Age", 14.0, 1),
        text_at(0.10, 0.22, "Alice", 2),
        text_at(0.30, 0.22, "30", 3),
    ];
    page.push(h_line(0.10, 0.05, 0.50, 4));
    page.push(h_line(0.17, 0.05, 0.50, 5));
    page.push(h_line(0.27, 0.05, 0.50, 6));
    page.push(v_line(0.05, 0.10, 0.27, 7));
    page.push(v_line(0.25, 0.10, 0.27, 8));
    page.push(v_line(0.50, 0.10, 0.27, 9));

    let detector = TableDetector::default();
    let hints = detector.detect(&page, &ctx());

    let headers: Vec<&DetectedHint> = hints
        .iter()
        .filter(|h| matches!(h.hint.kind, HintKind::TableHeader { .. }))
        .collect();
    assert_eq!(
        headers.len(),
        2,
        "Larger font row 0 should be detected as header"
    );
}

#[test]
fn no_header_when_all_same_style() {
    // All cells have the same font/size/bold → no header detection.
    let mut page = vec![
        text_at(0.10, 0.12, "A1", 0),
        text_at(0.30, 0.12, "B1", 1),
        text_at(0.10, 0.22, "A2", 2),
        text_at(0.30, 0.22, "B2", 3),
    ];
    page.push(h_line(0.10, 0.05, 0.50, 4));
    page.push(h_line(0.17, 0.05, 0.50, 5));
    page.push(h_line(0.27, 0.05, 0.50, 6));
    page.push(v_line(0.05, 0.10, 0.27, 7));
    page.push(v_line(0.25, 0.10, 0.27, 8));
    page.push(v_line(0.50, 0.10, 0.27, 9));

    let detector = TableDetector::default();
    let hints = detector.detect(&page, &ctx());

    let headers: Vec<&DetectedHint> = hints
        .iter()
        .filter(|h| matches!(h.hint.kind, HintKind::TableHeader { .. }))
        .collect();
    assert!(
        headers.is_empty(),
        "Same-style rows should not trigger header detection"
    );
}
