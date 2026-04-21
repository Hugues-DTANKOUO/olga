use super::*;
// ---------------------------------------------------------------------------
// Inline metadata propagation
// ---------------------------------------------------------------------------

#[test]
fn link_url_propagated_to_paragraph_metadata() {
    let mut asm = Assembler::with_defaults();
    let prim = text_prim(0.1, "click here", 0)
        .with_hint(SemanticHint::from_format(HintKind::Paragraph))
        .with_hint(SemanticHint::from_format(HintKind::Link {
            url: "https://example.com".to_string(),
        }));
    let flush = page_flush(0, vec![prim]);
    asm.process_page(&flush);
    let result = asm.finish();

    let para = &result.root.children[0];
    assert_eq!(
        para.metadata.get("link_url").map(|s| s.as_str()),
        Some("https://example.com")
    );
}

#[test]
fn expanded_text_propagated_to_metadata() {
    let mut asm = Assembler::with_defaults();
    let prim = text_prim(0.1, "Fig. 1", 0)
        .with_hint(SemanticHint::from_format(HintKind::Paragraph))
        .with_hint(SemanticHint::from_format(HintKind::ExpandedText {
            text: "Figure 1".to_string(),
        }));
    let flush = page_flush(0, vec![prim]);
    asm.process_page(&flush);
    let result = asm.finish();

    assert_eq!(
        result.root.children[0]
            .metadata
            .get("expanded_text")
            .map(|s| s.as_str()),
        Some("Figure 1")
    );
}

#[test]
fn emphasis_strong_propagated_to_metadata() {
    let mut asm = Assembler::with_defaults();
    let prim = text_prim(0.1, "bold text", 0)
        .with_hint(SemanticHint::from_format(HintKind::Paragraph))
        .with_hint(SemanticHint::from_format(HintKind::Strong));
    let flush = page_flush(0, vec![prim]);
    asm.process_page(&flush);
    let result = asm.finish();

    assert_eq!(
        result.root.children[0]
            .metadata
            .get("emphasis")
            .map(|s| s.as_str()),
        Some("bold")
    );
}

#[test]
fn link_on_footnote_propagated() {
    let mut asm = Assembler::with_defaults();
    let prim = text_prim(0.9, "ref", 0)
        .with_hint(SemanticHint::from_format(HintKind::Footnote {
            id: "fn1".to_string(),
        }))
        .with_hint(SemanticHint::from_format(HintKind::Link {
            url: "https://docs.example.com".to_string(),
        }));
    let flush = page_flush(0, vec![prim]);
    asm.process_page(&flush);
    let result = asm.finish();

    assert_eq!(
        result.root.children[0]
            .metadata
            .get("link_url")
            .map(|s| s.as_str()),
        Some("https://docs.example.com")
    );
}

// ---------------------------------------------------------------------------
// Confidence propagation
// ---------------------------------------------------------------------------

#[test]
fn paragraph_confidence_tracks_minimum_across_primitives() {
    // Paragraph opened by hinted prim (0.95), then bare text (1.0 implicit).
    // Min should be 0.95.
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![
            hinted_conf(text_prim(0.1, "high", 0), HintKind::Paragraph, 0.95),
            text_prim(0.15, "also high", 1), // bare → merges, confidence 1.0
        ],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    assert!(
        (result.root.children[0].confidence - 0.95).abs() < 0.01,
        "paragraph confidence should be min: {}",
        result.root.children[0].confidence
    );
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn empty_text_primitive_not_emitted() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(0, vec![hinted(text_prim(0.1, "", 0), HintKind::Paragraph)]);
    asm.process_page(&flush);
    let result = asm.finish();

    assert!(result.root.children.is_empty());
}

#[test]
fn only_decorations_produces_empty_document() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![line_prim(0.1, 0), rect_prim(0.2, 1), line_prim(0.3, 2)],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    assert!(result.root.children.is_empty());
    assert!(result.warnings.is_empty());
}

#[test]
fn image_between_paragraphs() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![
            hinted(text_prim(0.05, "before", 0), HintKind::Paragraph),
            image_prim(0.2, Some("diagram"), 1),
            hinted(text_prim(0.6, "after", 2), HintKind::Paragraph),
        ],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    assert_eq!(result.root.children.len(), 3);
    assert!(matches!(
        result.root.children[0].kind,
        NodeKind::Paragraph { .. }
    ));
    assert!(matches!(
        result.root.children[1].kind,
        NodeKind::Image { .. }
    ));
    assert!(matches!(
        result.root.children[2].kind,
        NodeKind::Paragraph { .. }
    ));
}

// ===========================================================================
