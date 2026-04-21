use super::*;

#[test]
fn single_paragraph_from_hint_plus_bare() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![
            hinted(text_prim(0.1, "Hello", 0), HintKind::Paragraph),
            text_prim(0.15, "world", 1),
        ],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    assert_eq!(result.root.children.len(), 1);
    let para = &result.root.children[0];
    assert_eq!(para.text(), Some("Hello world"));
    assert_eq!(para.confidence, 1.0);
}

#[test]
fn bare_text_becomes_implicit_paragraph() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(0, vec![text_prim(0.1, "bare text", 0)]);
    asm.process_page(&flush);
    let result = asm.finish();

    assert_eq!(result.root.children.len(), 1);
    assert_eq!(result.root.children[0].text(), Some("bare text"));
}

#[test]
fn consecutive_paragraph_hints_create_separate_paragraphs() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![
            hinted(text_prim(0.1, "line one", 0), HintKind::Paragraph),
            hinted(text_prim(0.15, "line two", 1), HintKind::Paragraph),
            hinted(text_prim(0.2, "line three", 2), HintKind::Paragraph),
        ],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    assert_eq!(result.root.children.len(), 3);
    assert_eq!(result.root.children[0].text(), Some("line one"));
    assert_eq!(result.root.children[1].text(), Some("line two"));
    assert_eq!(result.root.children[2].text(), Some("line three"));
}

#[test]
fn bare_text_merges_into_open_paragraph() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![
            hinted(text_prim(0.1, "line one", 0), HintKind::Paragraph),
            text_prim(0.15, "line two", 1),
            text_prim(0.2, "line three", 2),
        ],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    assert_eq!(result.root.children.len(), 1);
    assert_eq!(
        result.root.children[0].text(),
        Some("line one line two line three")
    );
}

#[test]
fn whitespace_aware_merge_no_double_spaces() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![
            hinted(text_prim(0.1, "word ", 0), HintKind::Paragraph),
            text_prim(0.15, "next", 1),
        ],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    assert_eq!(result.root.children[0].text(), Some("word next"));
}

#[test]
fn whitespace_aware_merge_leading_space() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![
            hinted(text_prim(0.1, "word", 0), HintKind::Paragraph),
            text_prim(0.15, " next", 1),
        ],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    assert_eq!(result.root.children[0].text(), Some("word next"));
}
