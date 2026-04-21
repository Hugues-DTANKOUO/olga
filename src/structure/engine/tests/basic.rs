use super::*;
use crate::model::NodeKind;

#[test]
fn empty_document_produces_empty_root() {
    let engine = StructureEngine::new(StructureConfig::default());
    let result = engine.structure(decode_result(vec![]));

    assert!(matches!(result.root.kind, NodeKind::Document));
    assert!(result.root.children.is_empty());
    assert!(result.heuristic_pages.is_empty());
    assert!(result.warnings.is_empty());
}

#[test]
fn single_hinted_paragraph() {
    let prims = vec![hinted(
        text_prim(0, 0.1, "Hello world", 0),
        HintKind::Paragraph,
    )];
    let engine = StructureEngine::new(StructureConfig::default());
    let result = engine.structure(decode_result(prims));

    assert!(matches!(result.root.kind, NodeKind::Document));
    assert_eq!(result.root.children.len(), 1);
    match &result.root.children[0].kind {
        NodeKind::Paragraph { text } => assert_eq!(text, "Hello world"),
        other => panic!("expected Paragraph, got {:?}", other),
    }
    assert!(result.heuristic_pages.is_empty());
}

#[test]
fn single_unhinted_text_with_detectors() {
    let prims = vec![text_prim(0, 0.1, "Bare text here", 0)];
    let engine = StructureEngine::new(StructureConfig::default()).with_default_detectors();
    let result = engine.structure(decode_result(prims));

    assert!(matches!(result.root.kind, NodeKind::Document));
    assert_eq!(result.root.children.len(), 1);
    match &result.root.children[0].kind {
        NodeKind::Paragraph { text } => assert_eq!(text, "Bare text here"),
        other => panic!("expected Paragraph, got {:?}", other),
    }
    assert_eq!(result.heuristic_pages, vec![0]);
}

#[test]
fn hinted_document_skips_detection() {
    let prims = vec![
        hinted(text_prim(0, 0.1, "Paragraph one", 0), HintKind::Paragraph),
        hinted(text_prim(0, 0.2, "Paragraph two", 1), HintKind::Paragraph),
    ];
    let engine = StructureEngine::new(StructureConfig::default()).with_default_detectors();
    let result = engine.structure(decode_result(prims));

    assert_eq!(result.root.children.len(), 2);
    assert_eq!(result.root.children[0].text(), Some("Paragraph one"));
    assert_eq!(result.root.children[1].text(), Some("Paragraph two"));
    assert!(result.heuristic_pages.is_empty());
}

#[test]
fn multi_page_document_assembled() {
    let prims = vec![
        hinted(text_prim(0, 0.1, "Page one text", 0), HintKind::Paragraph),
        hinted(text_prim(1, 0.1, "Page two text", 1), HintKind::Paragraph),
    ];
    let engine = StructureEngine::new(StructureConfig::default());
    let result = engine.structure(decode_result(prims));

    assert_eq!(result.root.children.len(), 2);
    match &result.root.children[0].kind {
        NodeKind::Paragraph { text } => assert_eq!(text, "Page one text"),
        other => panic!("expected Paragraph, got {:?}", other),
    }
    match &result.root.children[1].kind {
        NodeKind::Paragraph { text } => assert_eq!(text, "Page two text"),
        other => panic!("expected Paragraph, got {:?}", other),
    }
}

#[test]
fn heading_plus_paragraph_hinted() {
    let prims = vec![
        hinted(
            text_prim(0, 0.05, "Title", 0),
            HintKind::Heading { level: 1 },
        ),
        hinted(
            text_prim(0, 0.15, "Body text goes here.", 1),
            HintKind::Paragraph,
        ),
    ];
    let engine = StructureEngine::new(StructureConfig::default());
    let result = engine.structure(decode_result(prims));

    assert_eq!(result.root.children.len(), 2);
    match &result.root.children[0].kind {
        NodeKind::Heading { level, text } => {
            assert_eq!(*level, 1);
            assert_eq!(text, "Title");
        }
        other => panic!("expected Heading, got {:?}", other),
    }
    match &result.root.children[1].kind {
        NodeKind::Paragraph { text } => assert_eq!(text, "Body text goes here."),
        other => panic!("expected Paragraph, got {:?}", other),
    }
}

#[test]
fn bare_text_without_detectors_wraps_as_paragraph() {
    let prims = vec![text_prim(0, 0.1, "Bare text", 0)];
    let engine = StructureEngine::new(StructureConfig::default());
    let result = engine.structure(decode_result(prims));

    assert_eq!(result.root.children.len(), 1);
    match &result.root.children[0].kind {
        NodeKind::Paragraph { text } => assert_eq!(text, "Bare text"),
        other => panic!("expected Paragraph, got {:?}", other),
    }
    assert!(result.heuristic_pages.is_empty());
}
