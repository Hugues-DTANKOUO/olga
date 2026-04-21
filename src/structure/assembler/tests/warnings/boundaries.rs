use super::*;
// ---------------------------------------------------------------------------
// Table closes when next page has no table data
// ---------------------------------------------------------------------------

#[test]
fn table_closes_at_page_boundary_when_no_table_on_next_page() {
    let mut asm = Assembler::with_defaults();

    // Page 0: a 2-cell table
    let prims_p0 = vec![
        hinted(
            text_prim(0.1, "cell-A", 0),
            HintKind::TableCell {
                row: 0,
                col: 0,
                rowspan: 1,
                colspan: 1,
            },
        ),
        hinted(
            text_prim(0.15, "cell-B", 1),
            HintKind::TableCell {
                row: 0,
                col: 1,
                rowspan: 1,
                colspan: 1,
            },
        ),
    ];
    asm.process_page(&page_flush(0, prims_p0));

    // Page 1: just a paragraph (no table hints)
    let prims_p1 = vec![hinted(
        text_prim(0.1, "regular paragraph", 2),
        HintKind::Paragraph,
    )];
    asm.process_page(&page_flush(1, prims_p1));

    let result = asm.finish();

    // The table should be closed and the paragraph should be separate.
    let tables: Vec<_> = result
        .root
        .children
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::Table { .. }))
        .collect();
    let paragraphs: Vec<_> = result
        .root
        .children
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::Paragraph { .. }))
        .collect();
    assert_eq!(tables.len(), 1, "table should be closed and emitted");
    assert_eq!(paragraphs.len(), 1, "paragraph should follow the table");
}

// ---------------------------------------------------------------------------
// List closes when next page has no list data
// ---------------------------------------------------------------------------

#[test]
fn list_closes_at_page_boundary_when_no_list_on_next_page() {
    let mut asm = Assembler::with_defaults();

    // Page 0: list items
    let prims_p0 = vec![
        hinted(
            text_prim(0.1, "item 1", 0),
            HintKind::ListItem {
                depth: 0,
                ordered: false,
                list_group: None,
            },
        ),
        hinted(
            text_prim(0.15, "item 2", 1),
            HintKind::ListItem {
                depth: 0,
                ordered: false,
                list_group: None,
            },
        ),
    ];
    asm.process_page(&page_flush(0, prims_p0));

    // Page 1: just a paragraph
    let prims_p1 = vec![hinted(text_prim(0.1, "not a list", 2), HintKind::Paragraph)];
    asm.process_page(&page_flush(1, prims_p1));

    let result = asm.finish();

    let lists: Vec<_> = result
        .root
        .children
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::List { .. }))
        .collect();
    let paragraphs: Vec<_> = result
        .root
        .children
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::Paragraph { .. }))
        .collect();
    assert_eq!(lists.len(), 1, "list should be closed and emitted");
    assert_eq!(paragraphs.len(), 1, "paragraph should follow the list");
}
