use super::*;

#[test]
fn section_contains_children() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![
            hinted(
                text_prim(0.05, "", 0),
                HintKind::SectionStart {
                    title: Some("Intro".to_string()),
                },
            ),
            hinted(text_prim(0.1, "content", 1), HintKind::Paragraph),
            hinted(text_prim(0.2, "", 2), HintKind::SectionEnd),
        ],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    assert_eq!(result.root.children.len(), 1);
    match &result.root.children[0].kind {
        NodeKind::Section { level } => assert_eq!(*level, 1),
        other => panic!("expected Section, got {:?}", other),
    }
    assert_eq!(result.root.children[0].children.len(), 1);
    assert_eq!(result.root.children[0].children[0].text(), Some("content"));
}

#[test]
fn section_title_propagated_as_metadata() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![
            hinted(
                text_prim(0.05, "", 0),
                HintKind::SectionStart {
                    title: Some("Introduction".to_string()),
                },
            ),
            hinted(text_prim(0.1, "content", 1), HintKind::Paragraph),
            hinted(text_prim(0.2, "", 2), HintKind::SectionEnd),
        ],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    let section = &result.root.children[0];
    assert_eq!(
        section.metadata.get("title").map(|s| s.as_str()),
        Some("Introduction")
    );
}

#[test]
fn nested_sections() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![
            hinted(
                text_prim(0.02, "", 0),
                HintKind::SectionStart { title: None },
            ),
            hinted(text_prim(0.05, "outer", 1), HintKind::Paragraph),
            hinted(
                text_prim(0.1, "", 2),
                HintKind::SectionStart { title: None },
            ),
            hinted(text_prim(0.15, "inner", 3), HintKind::Paragraph),
            hinted(text_prim(0.2, "", 4), HintKind::SectionEnd),
            hinted(text_prim(0.25, "", 5), HintKind::SectionEnd),
        ],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    assert_eq!(result.root.children.len(), 1);
    let outer = &result.root.children[0];
    assert_eq!(outer.children.len(), 2);
    assert_eq!(outer.children[0].text(), Some("outer"));
    match &outer.children[1].kind {
        NodeKind::Section { level } => assert_eq!(*level, 2),
        other => panic!("expected inner Section, got {:?}", other),
    }
    assert_eq!(outer.children[1].children[0].text(), Some("inner"));
}

#[test]
fn section_end_without_start_warns() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(0, vec![hinted(text_prim(0.1, "", 0), HintKind::SectionEnd)]);
    asm.process_page(&flush);
    let result = asm.finish();

    assert_eq!(result.warnings.len(), 1);
    assert!(
        result.warnings[0]
            .message
            .contains("SectionEnd without matching")
    );
}

#[test]
fn unclosed_section_closed_on_finish() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![
            hinted(
                text_prim(0.05, "", 0),
                HintKind::SectionStart { title: None },
            ),
            hinted(text_prim(0.1, "orphaned", 1), HintKind::Paragraph),
        ],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    assert_eq!(result.root.children.len(), 1);
    assert_eq!(result.root.children[0].children[0].text(), Some("orphaned"));
}
