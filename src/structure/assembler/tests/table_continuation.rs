use super::*;
// ---------------------------------------------------------------------------
// Cross-page table continuation via TableCellContinuation
// ---------------------------------------------------------------------------

#[test]
fn table_continuation_merges_into_single_table() {
    let mut asm = Assembler::with_defaults();

    // Page 0: a 2×2 table with regular TableCell hints.
    asm.process_page(&page_flush(
        0,
        vec![
            hinted(
                text_prim_geo(0.05, 0.1, 0.4, 12.0, "H1", 0, 0),
                HintKind::TableCell {
                    row: 0,
                    col: 0,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
            hinted(
                text_prim_geo(0.55, 0.1, 0.4, 12.0, "H2", 0, 1),
                HintKind::TableCell {
                    row: 0,
                    col: 1,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
            hinted(
                text_prim_geo(0.05, 0.2, 0.4, 12.0, "A1", 0, 2),
                HintKind::TableCell {
                    row: 1,
                    col: 0,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
            hinted(
                text_prim_geo(0.55, 0.2, 0.4, 12.0, "A2", 0, 3),
                HintKind::TableCell {
                    row: 1,
                    col: 1,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
        ],
    ));

    // Page 1: continuation — rows start at 0 again but use
    // TableCellContinuation so the assembler offsets them.
    asm.process_page(&page_flush(
        1,
        vec![
            hinted(
                text_prim_geo(0.05, 0.1, 0.4, 12.0, "B1", 1, 4),
                HintKind::TableCellContinuation {
                    row: 0,
                    col: 0,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
            hinted(
                text_prim_geo(0.55, 0.1, 0.4, 12.0, "B2", 1, 5),
                HintKind::TableCellContinuation {
                    row: 0,
                    col: 1,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
            hinted(
                text_prim_geo(0.05, 0.2, 0.4, 12.0, "C1", 1, 6),
                HintKind::TableCellContinuation {
                    row: 1,
                    col: 0,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
            hinted(
                text_prim_geo(0.55, 0.2, 0.4, 12.0, "C2", 1, 7),
                HintKind::TableCellContinuation {
                    row: 1,
                    col: 1,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
        ],
    ));

    let result = asm.finish();

    // Should produce ONE table, not two.
    assert_eq!(
        result.root.children.len(),
        1,
        "continuation should merge into a single table"
    );
    match &result.root.children[0].kind {
        NodeKind::Table { rows, cols } => {
            // 2 original rows + 2 continuation rows = 4.
            assert_eq!(*rows, 4, "expected 4 rows (2 original + 2 continuation)");
            assert_eq!(*cols, 2);
        }
        other => panic!("expected Table, got {:?}", other),
    }
    // Spans both pages.
    assert_eq!(result.root.children[0].source_pages, 0..2);
}

#[test]
fn table_continuation_row_offset_is_correct() {
    let mut asm = Assembler::with_defaults();

    // Page 0: 3 rows (0, 1, 2).
    asm.process_page(&page_flush(
        0,
        vec![
            hinted(
                text_prim_geo(0.05, 0.1, 0.4, 12.0, "R0", 0, 0),
                HintKind::TableCell {
                    row: 0,
                    col: 0,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
            hinted(
                text_prim_geo(0.05, 0.2, 0.4, 12.0, "R1", 0, 1),
                HintKind::TableCell {
                    row: 1,
                    col: 0,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
            hinted(
                text_prim_geo(0.05, 0.3, 0.4, 12.0, "R2", 0, 2),
                HintKind::TableCell {
                    row: 2,
                    col: 0,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
        ],
    ));

    // Page 1: continuation with row 0 → should become row 3.
    asm.process_page(&page_flush(
        1,
        vec![hinted(
            text_prim_geo(0.05, 0.1, 0.4, 12.0, "R3", 1, 3),
            HintKind::TableCellContinuation {
                row: 0,
                col: 0,
                rowspan: 1,
                colspan: 1,
            },
        )],
    ));

    let result = asm.finish();
    assert_eq!(result.root.children.len(), 1);
    match &result.root.children[0].kind {
        NodeKind::Table { rows, cols } => {
            assert_eq!(*rows, 4, "3 original + 1 continuation = 4 rows");
            assert_eq!(*cols, 1);
        }
        other => panic!("expected Table, got {:?}", other),
    }

    // Verify the last cell text is "R3" and it's at row 3.
    // Structure is Table → TableRow → TableCell.
    let table = &result.root.children[0];
    let last_row = table
        .children
        .last()
        .expect("table should have row children");
    assert!(
        matches!(last_row.kind, NodeKind::TableRow),
        "expected TableRow, got {:?}",
        last_row.kind
    );
    let last_cell = last_row
        .children
        .last()
        .expect("row should have cell children");
    match &last_cell.kind {
        NodeKind::TableCell { row, col, text, .. } => {
            assert_eq!(*row, 3, "continuation row 0 should be offset to row 3");
            assert_eq!(*col, 0);
            assert!(text.contains("R3"), "cell text should be R3, got: {}", text);
        }
        other => panic!("expected TableCell, got {:?}", other),
    }
}

#[test]
fn table_continuation_keeps_table_open_at_page_boundary() {
    let mut asm = Assembler::with_defaults();

    // Page 0: start a table.
    asm.process_page(&page_flush(
        0,
        vec![hinted(
            text_prim_geo(0.05, 0.1, 0.4, 12.0, "A", 0, 0),
            HintKind::TableCell {
                row: 0,
                col: 0,
                rowspan: 1,
                colspan: 1,
            },
        )],
    ));

    // Page 1: continuation.
    asm.process_page(&page_flush(
        1,
        vec![hinted(
            text_prim_geo(0.05, 0.1, 0.4, 12.0, "B", 1, 1),
            HintKind::TableCellContinuation {
                row: 0,
                col: 0,
                rowspan: 1,
                colspan: 1,
            },
        )],
    ));

    // Page 2: NOT a continuation — just a paragraph.
    asm.process_page(&page_flush(
        2,
        vec![text_prim_geo(0.05, 0.1, 0.9, 12.0, "Normal text", 2, 2)],
    ));

    let result = asm.finish();

    // Table (pages 0-1) + paragraph (page 2).
    assert_eq!(
        result.root.children.len(),
        2,
        "should have table + paragraph"
    );
    assert!(
        matches!(result.root.children[0].kind, NodeKind::Table { .. }),
        "first child should be a table"
    );
    assert_eq!(result.root.children[0].source_pages, 0..2);
}
