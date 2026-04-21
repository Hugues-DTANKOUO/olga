use super::*;

#[test]
fn borderless_table_detected() {
    let page = vec![
        text_at(0.05, 0.10, "Name", 0),
        text_at(0.40, 0.10, "Age", 1),
        text_at(0.05, 0.15, "Alice", 2),
        text_at(0.40, 0.15, "30", 3),
        text_at(0.05, 0.20, "Bob", 4),
        text_at(0.40, 0.20, "25", 5),
    ];

    let detector = TableDetector::default();
    let hints = detector.detect(&page, &ctx());

    assert_eq!(hints.len(), 6, "Should detect 6 borderless table cells");
    for hint in &hints {
        assert!(matches!(hint.hint.kind, HintKind::TableCell { .. }));
        assert!(
            (hint.hint.confidence - 0.65).abs() < 0.01,
            "Stream confidence should be 0.65"
        );
    }
}

#[test]
fn single_column_not_a_table() {
    let page = vec![
        text_at(0.05, 0.10, "Line one", 0),
        text_at(0.05, 0.15, "Line two", 1),
        text_at(0.05, 0.20, "Line three", 2),
    ];

    let detector = TableDetector::default();
    let hints = detector.detect(&page, &ctx());
    assert!(hints.is_empty(), "Single column should not be a table");
}

#[test]
fn too_few_items_not_a_table() {
    let page = vec![text_at(0.05, 0.10, "One", 0), text_at(0.40, 0.10, "Two", 1)];

    let detector = TableDetector::default();
    let hints = detector.detect(&page, &ctx());
    assert!(hints.is_empty(), "Too few items for a table");
}

#[test]
fn empty_page_no_hints() {
    let page: Vec<Primitive> = vec![];
    let detector = TableDetector::default();
    let hints = detector.detect(&page, &ctx());
    assert!(hints.is_empty());
}

#[test]
fn already_hinted_skipped() {
    let page = vec![
        text_at(0.05, 0.10, "Cell", 0).with_hint(SemanticHint::from_format(HintKind::TableCell {
            row: 0,
            col: 0,
            rowspan: 1,
            colspan: 1,
        })),
        text_at(0.40, 0.10, "Cell", 1),
    ];

    let detector = TableDetector::default();
    let hints = detector.detect(&page, &ctx());
    assert!(hints.is_empty());
}
#[test]
fn stream_uneven_columns() {
    // Table where not every row has the same number of populated columns.
    // Row 0: 3 columns, Row 1: 2 columns (missing middle), Row 2: 3 columns.
    let page = vec![
        text_at(0.05, 0.10, "A", 0),
        text_at(0.25, 0.10, "B", 1),
        text_at(0.45, 0.10, "C", 2),
        text_at(0.05, 0.15, "D", 3),
        // No middle column in row 1
        text_at(0.45, 0.15, "F", 5),
        text_at(0.05, 0.20, "G", 6),
        text_at(0.25, 0.20, "H", 7),
        text_at(0.45, 0.20, "I", 8),
    ];

    let detector = TableDetector::default();
    let hints = detector.detect(&page, &ctx());

    // Rows with >= 2 columns should still be detected.
    assert!(
        hints.len() >= 6,
        "Uneven columns should still detect table cells, got {}",
        hints.len()
    );
}

#[test]
fn y_gap_splits_separate_tables() {
    // Two table regions separated by a large Y gap.
    let page = vec![
        // Table 1 (y=0.10..0.20)
        text_at(0.05, 0.10, "A1", 0),
        text_at(0.40, 0.10, "B1", 1),
        text_at(0.05, 0.15, "A2", 2),
        text_at(0.40, 0.15, "B2", 3),
        text_at(0.05, 0.20, "A3", 4),
        text_at(0.40, 0.20, "B3", 5),
        // Large gap (0.20 → 0.60 = 0.40 gap, median gap = 0.05)
        // Table 2 (y=0.60..0.70)
        text_at(0.05, 0.60, "C1", 6),
        text_at(0.40, 0.60, "D1", 7),
        text_at(0.05, 0.65, "C2", 8),
        text_at(0.40, 0.65, "D2", 9),
        text_at(0.05, 0.70, "C3", 10),
        text_at(0.40, 0.70, "D3", 11),
    ];

    let detector = TableDetector::default();
    let hints = detector.detect(&page, &ctx());

    assert_eq!(hints.len(), 12, "All items should be detected");

    // Verify two distinct tables by checking that row indices reset.
    // Table 1 primitives (indices 0-5) should have rows 0-2.
    // Table 2 primitives (indices 6-11) should ALSO have rows 0-2.
    // This proves segment_y_clusters split the Y space correctly.
    let table1_rows: Vec<u32> = hints
        .iter()
        .filter(|h| h.primitive_index <= 5)
        .filter_map(|h| match &h.hint.kind {
            HintKind::TableCell { row, .. } => Some(*row),
            HintKind::TableHeader { .. } => Some(0),
            _ => None,
        })
        .collect();
    let table2_rows: Vec<u32> = hints
        .iter()
        .filter(|h| h.primitive_index >= 6)
        .filter_map(|h| match &h.hint.kind {
            HintKind::TableCell { row, .. } => Some(*row),
            HintKind::TableHeader { .. } => Some(0),
            _ => None,
        })
        .collect();

    assert_eq!(table1_rows.len(), 6, "Table 1 should have 6 hints");
    assert_eq!(table2_rows.len(), 6, "Table 2 should have 6 hints");

    // Both tables must have row 0 entries (proving row indices reset).
    assert!(table1_rows.contains(&0), "Table 1 should contain row 0");
    assert!(
        table2_rows.contains(&0),
        "Table 2 should contain row 0 (proving split happened)"
    );

    // Max row in each table should be 2 (3 rows: 0, 1, 2).
    let t1_max = table1_rows.iter().max().copied().unwrap_or(0);
    let t2_max = table2_rows.iter().max().copied().unwrap_or(0);
    assert_eq!(t1_max, 2, "Table 1 should have rows 0-2");
    assert_eq!(t2_max, 2, "Table 2 should have rows 0-2");
}
