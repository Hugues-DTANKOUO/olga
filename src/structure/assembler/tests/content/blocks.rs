use super::*;
// ---------------------------------------------------------------------------
// CodeBlock and BlockQuote
// ---------------------------------------------------------------------------

#[test]
fn codeblock_assembled() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![hinted(
            text_prim(0.1, "fn main() {}", 0),
            HintKind::CodeBlock,
        )],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    match &result.root.children[0].kind {
        NodeKind::CodeBlock { text, .. } => assert_eq!(text, "fn main() {}"),
        other => panic!("expected CodeBlock, got {:?}", other),
    }
}

#[test]
fn codeblock_merges_with_newlines() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![
            hinted(text_prim(0.1, "line 1;", 0), HintKind::CodeBlock),
            hinted(text_prim(0.12, "line 2;", 1), HintKind::CodeBlock),
            hinted(text_prim(0.14, "line 3;", 2), HintKind::CodeBlock),
        ],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    match &result.root.children[0].kind {
        NodeKind::CodeBlock { text, .. } => {
            assert_eq!(text, "line 1;\nline 2;\nline 3;");
        }
        other => panic!("expected CodeBlock, got {:?}", other),
    }
}

#[test]
fn blockquote_assembled() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![hinted(
            text_prim(0.1, "To be or not to be", 0),
            HintKind::BlockQuote,
        )],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    match &result.root.children[0].kind {
        NodeKind::BlockQuote { text } => assert_eq!(text, "To be or not to be"),
        other => panic!("expected BlockQuote, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Footnotes
// ---------------------------------------------------------------------------

#[test]
fn footnote_emitted_as_leaf() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![hinted(
            text_prim(0.9, "See reference 1", 0),
            HintKind::Footnote {
                id: "fn1".to_string(),
            },
        )],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    match &result.root.children[0].kind {
        NodeKind::Footnote { id, text } => {
            assert_eq!(id, "fn1");
            assert_eq!(text, "See reference 1");
        }
        other => panic!("expected Footnote, got {:?}", other),
    }
}
