use super::*;
// ---------------------------------------------------------------------------
// Mixed document (heading + para + table + para)
// ---------------------------------------------------------------------------

#[test]
fn mixed_document_structure() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![
            hinted(text_prim(0.05, "Title", 0), HintKind::Heading { level: 1 }),
            hinted(text_prim(0.1, "intro text", 1), HintKind::Paragraph),
            hinted(
                text_prim(0.2, "cell", 2),
                HintKind::TableCell {
                    row: 0,
                    col: 0,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
            hinted(text_prim(0.4, "after table", 3), HintKind::Paragraph),
        ],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    assert_eq!(result.root.children.len(), 4);
    assert!(matches!(
        result.root.children[0].kind,
        NodeKind::Heading { .. }
    ));
    assert!(matches!(
        result.root.children[1].kind,
        NodeKind::Paragraph { .. }
    ));
    assert!(matches!(
        result.root.children[2].kind,
        NodeKind::Table { .. }
    ));
    assert!(matches!(
        result.root.children[3].kind,
        NodeKind::Paragraph { .. }
    ));
}
