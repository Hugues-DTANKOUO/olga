use super::*;
// ---------------------------------------------------------------------------
// Cross-page table continuity
// ---------------------------------------------------------------------------

#[test]
fn table_survives_page_boundary_with_hints() {
    let mut asm = Assembler::with_defaults();

    // Page 0: start a table with 2 rows.
    asm.process_page(&page_flush(
        0,
        vec![
            hinted(
                text_prim(0.1, "H1", 0),
                HintKind::TableCell {
                    row: 0,
                    col: 0,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
            hinted(
                text_prim(0.1, "H2", 1),
                HintKind::TableCell {
                    row: 0,
                    col: 1,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
            hinted(
                text_prim(0.2, "A1", 2),
                HintKind::TableCell {
                    row: 1,
                    col: 0,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
            hinted(
                text_prim(0.2, "A2", 3),
                HintKind::TableCell {
                    row: 1,
                    col: 1,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
        ],
    ));

    // Page 1: continue the table with more rows.
    asm.process_page(&page_flush(
        1,
        vec![
            hinted(
                text_prim(0.1, "B1", 4),
                HintKind::TableCell {
                    row: 2,
                    col: 0,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
            hinted(
                text_prim(0.1, "B2", 5),
                HintKind::TableCell {
                    row: 2,
                    col: 1,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
        ],
    ));

    let result = asm.finish();

    // Should produce a SINGLE table spanning 2 pages.
    assert_eq!(result.root.children.len(), 1);
    match &result.root.children[0].kind {
        NodeKind::Table { rows, cols } => {
            assert_eq!(*rows, 3);
            assert_eq!(*cols, 2);
        }
        other => panic!("expected Table, got {:?}", other),
    }
    assert_eq!(result.root.children[0].source_pages, 0..2);
}

#[test]
fn table_closes_when_next_page_has_no_table() {
    let mut asm = Assembler::with_defaults();

    // Page 0: a table.
    asm.process_page(&page_flush(
        0,
        vec![hinted(
            text_prim(0.1, "Cell", 0),
            HintKind::TableCell {
                row: 0,
                col: 0,
                rowspan: 1,
                colspan: 1,
            },
        )],
    ));

    // Page 1: a paragraph (not a table).
    asm.process_page(&page_flush(
        1,
        vec![hinted(
            text_prim(0.1, "After table", 1),
            HintKind::Paragraph,
        )],
    ));

    let result = asm.finish();

    assert_eq!(result.root.children.len(), 2);
    assert!(matches!(
        result.root.children[0].kind,
        NodeKind::Table { .. }
    ));
    assert!(matches!(
        result.root.children[1].kind,
        NodeKind::Paragraph { .. }
    ));
}
