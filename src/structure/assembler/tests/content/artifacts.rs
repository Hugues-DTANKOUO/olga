use super::*;
// ---------------------------------------------------------------------------
// Artifacts (PageHeader / PageFooter)
// ---------------------------------------------------------------------------

#[test]
fn page_header_stripped_when_configured() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![
            hinted(text_prim(0.02, "HEADER", 0), HintKind::PageHeader),
            hinted(text_prim(0.1, "body", 1), HintKind::Paragraph),
            hinted(text_prim(0.95, "FOOTER", 2), HintKind::PageFooter),
        ],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    assert_eq!(result.root.children.len(), 1);
    assert_eq!(result.root.children[0].text(), Some("body"));
    assert_eq!(result.artifacts.len(), 2);
}

#[test]
fn page_header_kept_when_strip_disabled() {
    let config = StructureConfig {
        strip_artifacts: false,
        ..Default::default()
    };
    let mut asm = Assembler::new(config);
    let flush = page_flush(
        0,
        vec![
            hinted(text_prim(0.02, "HEADER", 0), HintKind::PageHeader),
            hinted(text_prim(0.1, "body", 1), HintKind::Paragraph),
        ],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    assert_eq!(result.root.children.len(), 2);
    assert!(result.artifacts.is_empty());
}
