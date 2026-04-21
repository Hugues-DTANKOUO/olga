use super::*;
// ---------------------------------------------------------------------------
// Multi-page
// ---------------------------------------------------------------------------

#[test]
fn multi_page_closes_blocks_at_boundary() {
    let mut asm = Assembler::with_defaults();

    asm.process_page(&page_flush(
        0,
        vec![hinted(text_prim(0.1, "page zero", 0), HintKind::Paragraph)],
    ));

    asm.process_page(&page_flush(
        1,
        vec![hinted(text_prim(0.1, "page one", 1), HintKind::Paragraph)],
    ));

    let result = asm.finish();

    assert_eq!(result.root.children.len(), 2);
    assert_eq!(result.root.children[0].text(), Some("page zero"));
    assert_eq!(result.root.children[1].text(), Some("page one"));
    assert_eq!(result.root.children[0].source_pages, 0..1);
    assert_eq!(result.root.children[1].source_pages, 1..2);
}

#[test]
fn section_spans_pages() {
    let mut asm = Assembler::with_defaults();

    asm.process_page(&page_flush(
        0,
        vec![
            hinted(
                text_prim(0.05, "", 0),
                HintKind::SectionStart { title: None },
            ),
            hinted(text_prim(0.1, "page0", 1), HintKind::Paragraph),
        ],
    ));

    asm.process_page(&page_flush(
        1,
        vec![
            hinted(text_prim(0.1, "page1", 2), HintKind::Paragraph),
            hinted(text_prim(0.2, "", 3), HintKind::SectionEnd),
        ],
    ));

    let result = asm.finish();

    assert_eq!(result.root.children.len(), 1);
    let section = &result.root.children[0];
    assert_eq!(section.children.len(), 2);
    assert!(section.source_pages.start == 0);
    assert!(section.source_pages.end >= 2);
}

// ---------------------------------------------------------------------------
// DocumentTitle
// ---------------------------------------------------------------------------

#[test]
fn document_title_becomes_heading() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![hinted(
            text_prim(0.05, "My Document", 0),
            HintKind::DocumentTitle,
        )],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    match &result.root.children[0].kind {
        NodeKind::Heading { level, text } => {
            assert_eq!(*level, 1);
            assert_eq!(text, "My Document");
        }
        other => panic!("expected Heading from DocumentTitle, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Table closes when non-table hint arrives
// ---------------------------------------------------------------------------

#[test]
fn table_closes_on_paragraph() {
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
            hinted(text_prim(0.3, "after", 1), HintKind::Paragraph),
        ],
    );
    asm.process_page(&flush);
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

// ---------------------------------------------------------------------------
// Inline hints don't break block structure
// ---------------------------------------------------------------------------

#[test]
fn inline_hints_ignored_at_block_level() {
    let mut asm = Assembler::with_defaults();
    let prim = text_prim(0.1, "bold text", 0)
        .with_hint(SemanticHint::from_format(HintKind::Paragraph))
        .with_hint(SemanticHint::from_format(HintKind::Strong));
    let flush = page_flush(0, vec![prim]);
    asm.process_page(&flush);
    let result = asm.finish();

    assert_eq!(result.root.children.len(), 1);
    assert_eq!(result.root.children[0].text(), Some("bold text"));
}
