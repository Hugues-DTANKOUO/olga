use super::*;
// ---------------------------------------------------------------------------
// Row/col span mismatch warning
// ---------------------------------------------------------------------------

#[test]
fn route_to_table_warns_on_span_mismatch() {
    let mut asm = Assembler::with_defaults();

    // Two primitives at same (row, col) with different rowspan.
    // Use (1,0) so the second TableCell at (1,0) doesn't trigger
    // the table-boundary detection (which only fires on (0,0)).
    let prims = vec![
        hinted(
            text_prim(0.1, "header", 0),
            HintKind::TableCell {
                row: 0,
                col: 0,
                rowspan: 1,
                colspan: 1,
            },
        ),
        hinted(
            text_prim(0.11, "first", 1),
            HintKind::TableCell {
                row: 1,
                col: 0,
                rowspan: 1,
                colspan: 1,
            },
        ),
        hinted(
            text_prim(0.12, "second", 2),
            HintKind::TableCell {
                row: 1,
                col: 0,
                rowspan: 2,
                colspan: 1,
            },
        ),
    ];
    asm.process_page(&page_flush(0, prims));
    let result = asm.finish();

    // Should emit a span mismatch warning.
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.message.contains("span mismatch")),
        "expected span mismatch warning, got: {:?}",
        result.warnings
    );
}

// ---------------------------------------------------------------------------
// Unclosed sections emit warning in finish()
// ---------------------------------------------------------------------------

#[test]
fn unclosed_sections_emit_warning() {
    let mut asm = Assembler::with_defaults();

    let prims = vec![
        hinted(
            text_prim(0.05, "before", 0),
            HintKind::SectionStart {
                title: Some("Section A".to_string()),
            },
        ),
        hinted(text_prim(0.1, "content a", 1), HintKind::Paragraph),
        hinted(
            text_prim(0.2, "nested", 2),
            HintKind::SectionStart {
                title: Some("Section B".to_string()),
            },
        ),
        hinted(text_prim(0.25, "content b", 3), HintKind::Paragraph),
        // No SectionEnd for either section.
    ];
    asm.process_page(&page_flush(0, prims));
    let result = asm.finish();

    // Should warn about 2 unclosed sections.
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.message.contains("2 section(s) still open")),
        "expected unclosed section warning, got: {:?}",
        result.warnings
    );

    // Both sections should still be emitted in the tree.
    let sections: Vec<_> = result
        .root
        .children
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::Section { .. }))
        .collect();
    assert!(
        !sections.is_empty(),
        "sections should be emitted even when auto-closed"
    );
}

// ---------------------------------------------------------------------------
// Repeated header with trailing whitespace
// ---------------------------------------------------------------------------

#[test]
fn repeated_header_detected_with_trailing_whitespace() {
    let mut asm = Assembler::with_defaults();

    // Page 0: table with header row (clean text).
    let prims_p0 = vec![
        hinted(
            text_prim(0.05, "Name", 0),
            HintKind::TableCell {
                row: 0,
                col: 0,
                rowspan: 1,
                colspan: 1,
            },
        ),
        hinted(
            text_prim(0.05, "Value", 1),
            HintKind::TableCell {
                row: 0,
                col: 1,
                rowspan: 1,
                colspan: 1,
            },
        ),
        hinted(
            text_prim(0.1, "Alice", 2),
            HintKind::TableCell {
                row: 1,
                col: 0,
                rowspan: 1,
                colspan: 1,
            },
        ),
        hinted(
            text_prim(0.1, "100", 3),
            HintKind::TableCell {
                row: 1,
                col: 1,
                rowspan: 1,
                colspan: 1,
            },
        ),
    ];
    asm.process_page(&page_flush(0, prims_p0));

    // Page 1: repeated header with trailing whitespace, then data.
    let prims_p1 = vec![
        hinted(
            text_prim(0.05, "Name ", 4), // trailing space
            HintKind::TableCell {
                row: 2,
                col: 0,
                rowspan: 1,
                colspan: 1,
            },
        ),
        hinted(
            text_prim(0.05, " Value", 5), // leading space
            HintKind::TableCell {
                row: 2,
                col: 1,
                rowspan: 1,
                colspan: 1,
            },
        ),
        hinted(
            text_prim(0.1, "Bob", 6),
            HintKind::TableCell {
                row: 3,
                col: 0,
                rowspan: 1,
                colspan: 1,
            },
        ),
        hinted(
            text_prim(0.1, "200", 7),
            HintKind::TableCell {
                row: 3,
                col: 1,
                rowspan: 1,
                colspan: 1,
            },
        ),
    ];
    asm.process_page(&page_flush(1, prims_p1));

    let result = asm.finish();

    // The table should have cells for rows 0, 1, 3 but NOT the repeated
    // header row 2 (which should have been detected and skipped).
    let tables: Vec<_> = result
        .root
        .children
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::Table { .. }))
        .collect();
    assert_eq!(tables.len(), 1);

    let cells: Vec<_> = tables[0]
        .children
        .iter()
        .flat_map(|row| row.children.iter())
        .filter(|n| matches!(n.kind, NodeKind::TableCell { .. }))
        .collect();

    // "Name " and " Value" on page 1 should be skipped (repeated header).
    // We expect: Name, Value, Alice, 100, Bob, 200 = 6 cells.
    let cell_texts: Vec<&str> = cells.iter().filter_map(|c| c.text()).collect();
    assert_eq!(
        cell_texts.len(),
        6,
        "expected 6 cells (header skipped), got: {:?}",
        cell_texts
    );
    assert!(
        cell_texts.iter().any(|t| t.contains("Bob")),
        "data row should survive; cells: {:?}",
        cell_texts
    );
}
