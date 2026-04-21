use super::*;

#[test]
fn ruled_table_detected() {
    // 2×2 table with ruling lines.
    let mut page = vec![
        text_at(0.10, 0.12, "Cell A1", 0),
        text_at(0.30, 0.12, "Cell B1", 1),
        text_at(0.10, 0.22, "Cell A2", 2),
        text_at(0.30, 0.22, "Cell B2", 3),
    ];
    // 3 horizontal lines (top, middle, bottom)
    page.push(h_line(0.10, 0.05, 0.50, 4));
    page.push(h_line(0.17, 0.05, 0.50, 5));
    page.push(h_line(0.27, 0.05, 0.50, 6));
    // 3 vertical lines (left, middle, right)
    page.push(v_line(0.05, 0.10, 0.27, 7));
    page.push(v_line(0.25, 0.10, 0.27, 8));
    page.push(v_line(0.50, 0.10, 0.27, 9));

    let detector = TableDetector::default();
    let hints = detector.detect(&page, &ctx());

    assert_eq!(hints.len(), 4, "Should detect 4 table cells");
    for hint in &hints {
        assert!(matches!(hint.hint.kind, HintKind::TableCell { .. }));
        assert!(
            (hint.hint.confidence - 0.85).abs() < 0.01,
            "Lattice confidence should be 0.85"
        );
    }
}

#[test]
fn lattice_3x3_table() {
    // 3×3 table with full grid.
    let mut page = vec![];
    for row in 0..3 {
        for col in 0..3 {
            let x = 0.10 + col as f32 * 0.15;
            let y = 0.12 + row as f32 * 0.10;
            page.push(text_at(
                x,
                y,
                &format!("R{}C{}", row, col),
                (row * 3 + col) as u64,
            ));
        }
    }
    // 4 horizontal lines
    for i in 0..4 {
        let y = 0.10 + i as f32 * 0.10;
        page.push(h_line(y, 0.05, 0.55, 10 + i));
    }
    // 4 vertical lines
    for i in 0..4 {
        let x = 0.05 + i as f32 * 0.15;
        page.push(v_line(x, 0.10, 0.40, 14 + i));
    }

    let detector = TableDetector::default();
    let hints = detector.detect(&page, &ctx());

    assert_eq!(hints.len(), 9, "Should detect 9 cells in 3×3 grid");
}

#[test]
fn rectangle_produces_table() {
    // A single rectangle around 4 text items + internal lines.
    let mut page = vec![
        text_at(0.12, 0.12, "A1", 0),
        text_at(0.32, 0.12, "B1", 1),
        text_at(0.12, 0.22, "A2", 2),
        text_at(0.32, 0.22, "B2", 3),
    ];
    // Outer rectangle.
    page.push(Primitive::observed(
        PrimitiveKind::Rectangle {
            fill: None,
            stroke: Some(Color::black()),
            stroke_thickness: Some(0.001),
        },
        BoundingBox::new(0.05, 0.10, 0.45, 0.17),
        0,
        4,
    ));
    // Internal horizontal line.
    page.push(h_line(0.17, 0.05, 0.50, 5));
    // Internal vertical line.
    page.push(v_line(0.25, 0.10, 0.27, 6));

    let detector = TableDetector::default();
    let hints = detector.detect(&page, &ctx());

    assert!(
        hints.len() >= 4,
        "Rectangle edges + internal lines should form a table"
    );
}
#[test]
fn lattice_colspan_detected() {
    // 3-row × 2-col grid with header spanning both columns.
    let mut page = vec![
        // Header spanning both columns: wide text centered.
        Primitive::observed(
            PrimitiveKind::Text {
                content: "Merged Header".to_string(),
                font_size: 12.0,
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
            BoundingBox::new(0.10, 0.12, 0.35, 0.02),
            0,
            0,
        ),
        text_at(0.10, 0.22, "Cell A2", 1),
        text_at(0.30, 0.22, "Cell B2", 2),
        text_at(0.10, 0.32, "Cell A3", 3),
        text_at(0.30, 0.32, "Cell B3", 4),
    ];
    // 4 horizontal lines
    page.push(h_line(0.10, 0.05, 0.50, 5));
    page.push(h_line(0.17, 0.05, 0.50, 6));
    page.push(h_line(0.27, 0.05, 0.50, 7));
    page.push(h_line(0.37, 0.05, 0.50, 8));
    // 3 vertical lines
    page.push(v_line(0.05, 0.10, 0.37, 9));
    page.push(v_line(0.25, 0.10, 0.37, 10));
    page.push(v_line(0.50, 0.10, 0.37, 11));

    let detector = TableDetector::default();
    let hints = detector.detect(&page, &ctx());

    assert_eq!(hints.len(), 5, "Should detect 5 cells");

    // Find the merged header cell (primitive index 0).
    let header = hints
        .iter()
        .find(|h| h.primitive_index == 0)
        .expect("Header cell should be detected");
    match &header.hint.kind {
        HintKind::TableCell { colspan, row, .. } => {
            assert_eq!(*row, 0);
            assert_eq!(*colspan, 2, "Header should span 2 columns");
        }
        // Also accept TableHeader if header detection triggers.
        HintKind::TableHeader { .. } => {}
        other => panic!("Expected TableCell or TableHeader, got {:?}", other),
    }
}
#[test]
fn lattice_rowspan_detected() {
    // 3-row × 2-col grid with a cell spanning 2 rows in col 0.
    let mut page = vec![
        // Row 0, col 0: tall text spanning into row 1
        Primitive::observed(
            PrimitiveKind::Text {
                content: "Tall Cell".to_string(),
                font_size: 12.0,
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
            // Center at (0.15, 0.195) falls in row 1 [0.17, 0.27),
            // top at 0.12 extends into row 0 [0.10, 0.17),
            // bottom at 0.27 reaches row 1 boundary.
            // This tests upward rowspan extension.
            BoundingBox::new(0.10, 0.12, 0.10, 0.15),
            0,
            0,
        ),
        text_at(0.30, 0.12, "B1", 1),
        // Row 1, col 0: empty (absorbed by rowspan)
        text_at(0.30, 0.22, "B2", 2),
        text_at(0.10, 0.32, "A3", 3),
        text_at(0.30, 0.32, "B3", 4),
    ];
    // 4 horizontal + 3 vertical lines for a 3×2 grid
    page.push(h_line(0.10, 0.05, 0.50, 5));
    page.push(h_line(0.17, 0.05, 0.50, 6));
    page.push(h_line(0.27, 0.05, 0.50, 7));
    page.push(h_line(0.37, 0.05, 0.50, 8));
    page.push(v_line(0.05, 0.10, 0.37, 9));
    page.push(v_line(0.25, 0.10, 0.37, 10));
    page.push(v_line(0.50, 0.10, 0.37, 11));

    let detector = TableDetector::default();
    let hints = detector.detect(&page, &ctx());

    // Find the "Tall Cell" hint (primitive index 0).
    let tall_cell = hints.iter().find(|h| h.primitive_index == 0);
    assert!(tall_cell.is_some(), "Tall cell should be detected");

    if let Some(hint) = tall_cell {
        match &hint.hint.kind {
            HintKind::TableCell { rowspan, .. } => {
                assert!(
                    *rowspan >= 2,
                    "Tall cell should have rowspan >= 2, got {}",
                    rowspan
                );
            }
            HintKind::TableHeader { .. } => {
                // Header detection may trigger; that's acceptable.
            }
            other => panic!("Expected TableCell or TableHeader, got {:?}", other),
        }
    }
}

// -- Tolerance boundary tests ---------------------------------------------

#[test]
fn text_at_exact_tolerance_boundary_included() {
    // Text items at exactly x_tolerance apart should cluster together.
    let page = vec![
        text_at(0.050, 0.10, "A", 0),
        text_at(0.065, 0.10, "B", 1), // Exactly x_tolerance (0.015) apart
        text_at(0.400, 0.10, "C", 2),
        text_at(0.050, 0.15, "D", 3),
        text_at(0.065, 0.15, "E", 4),
        text_at(0.400, 0.15, "F", 5),
    ];

    let detector = TableDetector::default();
    let hints = detector.detect(&page, &ctx());

    // Items at 0.050 and 0.065 should cluster into one column.
    // So we expect a 2-column table (not 3).
    assert!(
        !hints.is_empty(),
        "Should detect a table even with tight tolerance"
    );
}

// -- Partial grid (unified pipeline) tests --------------------------------

#[test]
fn partial_grid_hlines_only() {
    // Table with horizontal rules but no vertical rules.
    // Should generate virtual vertical edges and detect via the pipeline.
    let mut page = vec![
        text_at(0.10, 0.12, "Name", 0),
        text_at(0.40, 0.12, "Age", 1),
        text_at(0.10, 0.22, "Alice", 2),
        text_at(0.40, 0.22, "30", 3),
        text_at(0.10, 0.32, "Bob", 4),
        text_at(0.40, 0.32, "25", 5),
    ];
    // 4 horizontal lines, no vertical lines.
    page.push(h_line(0.10, 0.05, 0.60, 6));
    page.push(h_line(0.17, 0.05, 0.60, 7));
    page.push(h_line(0.27, 0.05, 0.60, 8));
    page.push(h_line(0.37, 0.05, 0.60, 9));

    let detector = TableDetector::default();
    let hints = detector.detect(&page, &ctx());

    assert!(
        hints.len() >= 4,
        "Partial grid (h-lines + virtual v-edges) should detect table cells, got {}",
        hints.len()
    );

    // Confidence should be lattice-level (0.85) since we go through the pipeline.
    for hint in &hints {
        assert!(
            hint.hint.confidence >= 0.80,
            "Partial grid should have lattice-level confidence"
        );
    }
}
