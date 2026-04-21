use super::*;
// ---------------------------------------------------------------------------
// Image handling (bare images → Image nodes)
// ---------------------------------------------------------------------------

#[test]
fn bare_image_emits_image_node() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(0, vec![image_prim(0.2, Some("A cat photo"), 0)]);
    asm.process_page(&flush);
    let result = asm.finish();

    assert_eq!(result.root.children.len(), 1);
    match &result.root.children[0].kind {
        NodeKind::Image { alt_text, format } => {
            assert_eq!(alt_text.as_deref(), Some("A cat photo"));
            assert_eq!(format, "PNG");
        }
        other => panic!("expected Image, got {:?}", other),
    }
}

#[test]
fn bare_image_no_alt_text() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(0, vec![image_prim(0.2, None, 0)]);
    asm.process_page(&flush);
    let result = asm.finish();

    match &result.root.children[0].kind {
        NodeKind::Image { alt_text, .. } => {
            assert!(alt_text.is_none());
        }
        other => panic!("expected Image, got {:?}", other),
    }
}

#[test]
fn hinted_image_uses_hint_role() {
    // An image with a Paragraph hint should be treated as paragraph, not image.
    let mut asm = Assembler::with_defaults();
    let prim = hinted(image_prim(0.2, Some("alt"), 0), HintKind::Paragraph);
    let flush = page_flush(0, vec![prim]);
    asm.process_page(&flush);
    let result = asm.finish();

    // Should be a paragraph (with alt text as content), not an Image node.
    assert!(matches!(
        result.root.children[0].kind,
        NodeKind::Paragraph { .. }
    ));
}

// ---------------------------------------------------------------------------
// Decoration (Line, Rectangle) handling — skip silently
// ---------------------------------------------------------------------------

#[test]
fn bare_line_acts_as_barrier() {
    let mut asm = Assembler::with_defaults();
    // A decoration line between two text blocks acts as a BARRIER:
    // the two texts are separated into distinct paragraphs.
    // This correctly handles horizontal rules, table borders, etc.
    let flush = page_flush(
        0,
        vec![
            text_prim(0.10, "before", 0), // bare text
            line_prim(0.12, 1),           // decoration — barrier
            text_prim(0.13, "after", 2),  // bare text — new paragraph
        ],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    // Line should NOT appear in the tree, but texts are SEPARATED.
    assert_eq!(result.root.children.len(), 2);
    assert_eq!(result.root.children[0].text(), Some("before"));
    assert_eq!(result.root.children[1].text(), Some("after"));
}

#[test]
fn bare_rectangle_skipped_silently() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(0, vec![rect_prim(0.1, 0)]);
    asm.process_page(&flush);
    let result = asm.finish();

    assert!(result.root.children.is_empty());
}

#[test]
fn decoration_between_different_blocks_does_not_merge() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![
            hinted(text_prim(0.1, "para", 0), HintKind::Paragraph),
            line_prim(0.15, 1),
            hinted(text_prim(0.2, "heading", 2), HintKind::Heading { level: 2 }),
        ],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    assert_eq!(result.root.children.len(), 2);
    assert!(matches!(
        result.root.children[0].kind,
        NodeKind::Paragraph { .. }
    ));
    assert!(matches!(
        result.root.children[1].kind,
        NodeKind::Heading { .. }
    ));
}

// ---------------------------------------------------------------------------
// Sidebar handling
// ---------------------------------------------------------------------------

#[test]
fn sidebar_emitted_as_paragraph_with_role() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![hinted(
            text_prim(0.1, "sidebar content", 0),
            HintKind::Sidebar,
        )],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    assert_eq!(result.root.children.len(), 1);
    assert_eq!(result.root.children[0].text(), Some("sidebar content"));
    assert_eq!(
        result.root.children[0]
            .metadata
            .get("role")
            .map(|s| s.as_str()),
        Some("sidebar")
    );
}

#[test]
fn empty_sidebar_skipped() {
    let mut asm = Assembler::with_defaults();
    // Image has no text content for sidebar — empty.
    let flush = page_flush(0, vec![hinted(text_prim(0.1, "", 0), HintKind::Sidebar)]);
    asm.process_page(&flush);
    let result = asm.finish();

    assert!(result.root.children.is_empty());
}
