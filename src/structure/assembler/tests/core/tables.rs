use super::*;

#[test]
fn table_from_cell_hints() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![
            hinted(
                text_prim(0.1, "A1", 0),
                HintKind::TableCell {
                    row: 0,
                    col: 0,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
            hinted(
                text_prim(0.1, "B1", 1),
                HintKind::TableCell {
                    row: 0,
                    col: 1,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
            hinted(
                text_prim(0.2, "A2", 2),
                HintKind::TableCell {
                    row: 1,
                    col: 0,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
            hinted(
                text_prim(0.2, "B2", 3),
                HintKind::TableCell {
                    row: 1,
                    col: 1,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
        ],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    assert_eq!(result.root.children.len(), 1);
    let table = &result.root.children[0];
    match &table.kind {
        NodeKind::Table { rows, cols } => {
            assert_eq!(*rows, 2);
            assert_eq!(*cols, 2);
        }
        other => panic!("expected Table, got {:?}", other),
    }
    assert_eq!(table.children.len(), 2);
    assert_eq!(table.children[0].children.len(), 2);
    assert_eq!(table.children[1].children.len(), 2);
    assert_eq!(table.children[0].children[0].text(), Some("A1"));
    assert_eq!(table.children[0].children[1].text(), Some("B1"));
    assert_eq!(table.children[1].children[0].text(), Some("A2"));
    assert_eq!(table.children[1].children[1].text(), Some("B2"));
}

#[test]
fn table_with_colspan() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![hinted(
            text_prim(0.1, "merged", 0),
            HintKind::TableCell {
                row: 0,
                col: 0,
                rowspan: 1,
                colspan: 3,
            },
        )],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    let cell = &result.root.children[0].children[0].children[0];
    match &cell.kind {
        NodeKind::TableCell { colspan, text, .. } => {
            assert_eq!(*colspan, 3);
            assert_eq!(text, "merged");
        }
        other => panic!("expected TableCell, got {:?}", other),
    }
}

#[test]
fn table_header_marked_in_metadata() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![
            hinted(text_prim(0.1, "Name", 0), HintKind::TableHeader { col: 0 }),
            hinted(text_prim(0.1, "Age", 1), HintKind::TableHeader { col: 1 }),
            hinted(
                text_prim(0.2, "Alice", 2),
                HintKind::TableCell {
                    row: 1,
                    col: 0,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
            hinted(
                text_prim(0.2, "30", 3),
                HintKind::TableCell {
                    row: 1,
                    col: 1,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
        ],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    let table = &result.root.children[0];
    let hdr_row = &table.children[0];
    assert_eq!(
        hdr_row.children[0].metadata.get("role").map(|s| s.as_str()),
        Some("header")
    );
    assert_eq!(
        hdr_row.children[1].metadata.get("role").map(|s| s.as_str()),
        Some("header")
    );
    let data_row = &table.children[1];
    assert!(!data_row.children[0].metadata.contains_key("role"));
}

#[test]
fn table_confidence_tracks_minimum() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![
            hinted_conf(
                text_prim(0.1, "A", 0),
                HintKind::TableCell {
                    row: 0,
                    col: 0,
                    rowspan: 1,
                    colspan: 1,
                },
                0.9,
            ),
            hinted_conf(
                text_prim(0.1, "B", 1),
                HintKind::TableCell {
                    row: 0,
                    col: 1,
                    rowspan: 1,
                    colspan: 1,
                },
                0.7,
            ),
        ],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    let table = &result.root.children[0];
    assert!(
        (table.confidence - 0.7).abs() < 0.01,
        "table confidence should be min of cells: {}",
        table.confidence
    );
}
