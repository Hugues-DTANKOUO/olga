use super::*;
// ---------------------------------------------------------------------------
// Repeated table header detection
// ---------------------------------------------------------------------------

#[test]
fn repeated_table_header_skipped() {
    let mut asm = Assembler::with_defaults();

    // Page 0: table with header row + data row.
    asm.process_page(&page_flush(
        0,
        vec![
            hinted(
                text_prim(0.1, "Name", 0),
                HintKind::TableCell {
                    row: 0,
                    col: 0,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
            hinted(
                text_prim(0.1, "Age", 1),
                HintKind::TableCell {
                    row: 0,
                    col: 1,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
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
    ));

    // Page 1: repeated header + new data.
    asm.process_page(&page_flush(
        1,
        vec![
            hinted(
                text_prim(0.1, "Name", 4),
                HintKind::TableCell {
                    row: 2,
                    col: 0,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
            hinted(
                text_prim(0.1, "Age", 5),
                HintKind::TableCell {
                    row: 2,
                    col: 1,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
            hinted(
                text_prim(0.2, "Bob", 6),
                HintKind::TableCell {
                    row: 3,
                    col: 0,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
            hinted(
                text_prim(0.2, "25", 7),
                HintKind::TableCell {
                    row: 3,
                    col: 1,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
        ],
    ));

    let result = asm.finish();

    // Single table, 3 real rows (header + Alice + Bob), repeated header skipped.
    assert_eq!(result.root.children.len(), 1);
    match &result.root.children[0].kind {
        NodeKind::Table { rows, cols } => {
            assert_eq!(*rows, 3, "expected 3 rows (header skipped)");
            assert_eq!(*cols, 2);
        }
        other => panic!("expected Table, got {:?}", other),
    }
    // Metadata should record the repeated header page.
    assert_eq!(
        result.root.children[0]
            .metadata
            .get("repeated_header_pages"),
        Some(&"1".to_string())
    );
}
