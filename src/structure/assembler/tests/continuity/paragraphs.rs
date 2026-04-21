use super::*;
// ---------------------------------------------------------------------------
// Split paragraph merging
// ---------------------------------------------------------------------------

#[test]
fn paragraph_merges_across_page_when_continuation_text() {
    let mut asm = Assembler::with_defaults();

    // Page 0: bare text (no Paragraph hint — implicit paragraph).
    asm.process_page(&page_flush(
        0,
        vec![text_prim(0.1, "The quick brown fox", 0)],
    ));

    // Page 1: bare text starting with lowercase — continuation.
    asm.process_page(&page_flush(
        1,
        vec![text_prim(0.1, "jumped over the lazy dog.", 1)],
    ));

    let result = asm.finish();

    // Merged into a single paragraph.
    assert_eq!(result.root.children.len(), 1);
    match &result.root.children[0].kind {
        NodeKind::Paragraph { text } => {
            assert!(text.contains("fox"));
            assert!(text.contains("jumped"));
        }
        other => panic!("expected Paragraph, got {:?}", other),
    }
    // Spans both pages.
    assert_eq!(result.root.children[0].source_pages, 0..2);
}

#[test]
fn paragraph_does_not_merge_when_uppercase_start() {
    let mut asm = Assembler::with_defaults();

    asm.process_page(&page_flush(0, vec![text_prim(0.1, "End of first.", 0)]));

    // Page 1: starts with uppercase — new paragraph.
    asm.process_page(&page_flush(1, vec![text_prim(0.1, "Start of second.", 1)]));

    let result = asm.finish();

    assert_eq!(result.root.children.len(), 2);
}

#[test]
fn hinted_paragraph_does_not_merge_across_pages() {
    let mut asm = Assembler::with_defaults();

    asm.process_page(&page_flush(
        0,
        vec![hinted(
            text_prim(0.1, "First paragraph", 0),
            HintKind::Paragraph,
        )],
    ));

    // Even though "second" starts lowercase, the Paragraph hint means
    // a new paragraph — don't merge.
    asm.process_page(&page_flush(
        1,
        vec![hinted(
            text_prim(0.1, "second paragraph", 1),
            HintKind::Paragraph,
        )],
    ));

    let result = asm.finish();
    assert_eq!(result.root.children.len(), 2);
}

#[test]
fn paragraph_merges_on_continuation_punctuation() {
    let mut asm = Assembler::with_defaults();

    asm.process_page(&page_flush(0, vec![text_prim(0.1, "He said", 0)]));

    // Page 1: starts with comma — continuation.
    asm.process_page(&page_flush(1, vec![text_prim(0.1, ", and she agreed.", 1)]));

    let result = asm.finish();
    assert_eq!(result.root.children.len(), 1);
    match &result.root.children[0].kind {
        NodeKind::Paragraph { text } => assert!(text.contains("said") && text.contains(",")),
        other => panic!("expected Paragraph, got {:?}", other),
    }
}
