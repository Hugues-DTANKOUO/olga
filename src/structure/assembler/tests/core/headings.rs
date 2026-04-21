use super::*;

#[test]
fn heading_emitted_as_leaf() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![hinted(
            text_prim(0.1, "Chapter 1", 0),
            HintKind::Heading { level: 1 },
        )],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    assert_eq!(result.root.children.len(), 1);
    match &result.root.children[0].kind {
        NodeKind::Heading { level, text } => {
            assert_eq!(*level, 1);
            assert_eq!(text, "Chapter 1");
        }
        other => panic!("expected Heading, got {:?}", other),
    }
}

#[test]
fn heading_breaks_paragraph_accumulation() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![
            hinted(text_prim(0.1, "intro", 0), HintKind::Paragraph),
            hinted(text_prim(0.2, "Title", 1), HintKind::Heading { level: 1 }),
            hinted(text_prim(0.3, "body", 2), HintKind::Paragraph),
        ],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    assert_eq!(result.root.children.len(), 3);
    assert_eq!(result.root.children[0].text(), Some("intro"));
    assert_eq!(result.root.children[1].text(), Some("Title"));
    assert_eq!(result.root.children[2].text(), Some("body"));
}
