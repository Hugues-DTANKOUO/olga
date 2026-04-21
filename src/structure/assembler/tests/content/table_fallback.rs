use super::*;
// ---------------------------------------------------------------------------
// ContainedInTableCell / FlattenedFromTableCell fallback
// ---------------------------------------------------------------------------

#[test]
fn contained_in_table_cell_routes_to_open_table() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![
            hinted(
                text_prim(0.1, "cell", 0),
                HintKind::TableCell {
                    row: 0,
                    col: 0,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
            // This primitive only has ContainedInTableCell — should route to table.
            text_prim(0.12, "nested", 1).with_hint(SemanticHint::from_format(
                HintKind::ContainedInTableCell {
                    row: 0,
                    col: 0,
                    depth: 0,
                },
            )),
        ],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    assert_eq!(result.root.children.len(), 1);
    let table = &result.root.children[0];
    assert!(matches!(table.kind, NodeKind::Table { .. }));
    // Cell (0,0) should contain "cell nested".
    assert_eq!(table.children[0].children[0].text(), Some("cell nested"));
}

#[test]
fn contained_in_table_cell_without_open_table_becomes_paragraph() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![
            text_prim(0.1, "orphaned", 0).with_hint(SemanticHint::from_format(
                HintKind::ContainedInTableCell {
                    row: 0,
                    col: 0,
                    depth: 0,
                },
            )),
        ],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    // Should become an implicit paragraph since no table is open.
    assert_eq!(result.root.children.len(), 1);
    assert!(matches!(
        result.root.children[0].kind,
        NodeKind::Paragraph { .. }
    ));
    assert_eq!(result.root.children[0].text(), Some("orphaned"));
}

#[test]
fn flattened_from_table_cell_routes_to_open_table() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![
            hinted(
                text_prim(0.1, "main", 0),
                HintKind::TableCell {
                    row: 0,
                    col: 0,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
            text_prim(0.12, "flattened", 1).with_hint(SemanticHint::from_format(
                HintKind::FlattenedFromTableCell {
                    row: 0,
                    col: 0,
                    depth: 1,
                },
            )),
        ],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    let table = &result.root.children[0];
    assert_eq!(table.children[0].children[0].text(), Some("main flattened"));
}
