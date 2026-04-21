use super::*;
use crate::model::NodeKind;

#[test]
fn table_assembly_through_engine() {
    let prims = vec![
        hinted(
            text_prim(0, 0.1, "Header", 0),
            HintKind::TableCell {
                row: 0,
                col: 0,
                rowspan: 1,
                colspan: 1,
            },
        ),
        hinted(
            text_prim(0, 0.1, "Value", 1),
            HintKind::TableCell {
                row: 0,
                col: 1,
                rowspan: 1,
                colspan: 1,
            },
        ),
        hinted(
            text_prim(0, 0.2, "Row1", 2),
            HintKind::TableCell {
                row: 1,
                col: 0,
                rowspan: 1,
                colspan: 1,
            },
        ),
        hinted(
            text_prim(0, 0.2, "Data1", 3),
            HintKind::TableCell {
                row: 1,
                col: 1,
                rowspan: 1,
                colspan: 1,
            },
        ),
    ];
    let engine = StructureEngine::new(StructureConfig::default());
    let result = engine.structure(decode_result(prims));

    assert_eq!(result.root.children.len(), 1);
    match &result.root.children[0].kind {
        NodeKind::Table { rows, cols } => {
            assert_eq!(*rows, 2);
            assert_eq!(*cols, 2);
        }
        other => panic!("expected Table, got {:?}", other),
    }
}

#[test]
fn list_assembly_through_engine() {
    let prims = vec![
        hinted(
            text_prim(0, 0.1, "Item one", 0),
            HintKind::ListItem {
                depth: 0,
                ordered: false,
                list_group: None,
            },
        ),
        hinted(
            text_prim(0, 0.2, "Item two", 1),
            HintKind::ListItem {
                depth: 0,
                ordered: false,
                list_group: None,
            },
        ),
    ];
    let engine = StructureEngine::new(StructureConfig::default());
    let result = engine.structure(decode_result(prims));

    assert_eq!(result.root.children.len(), 1);
    match &result.root.children[0].kind {
        NodeKind::List { ordered } => assert!(!ordered),
        other => panic!("expected List, got {:?}", other),
    }
}

#[test]
fn section_nesting_through_engine() {
    let prims = vec![
        hinted(
            text_prim(0, 0.05, "Section Start", 0),
            HintKind::SectionStart {
                title: Some("Introduction".to_string()),
            },
        ),
        hinted(text_prim(0, 0.15, "Section body.", 1), HintKind::Paragraph),
        hinted(text_prim(0, 0.25, "Section End", 2), HintKind::SectionEnd),
    ];
    let engine = StructureEngine::new(StructureConfig::default());
    let result = engine.structure(decode_result(prims));

    assert_eq!(result.root.children.len(), 1);
    match &result.root.children[0].kind {
        NodeKind::Section { level } => assert_eq!(*level, 1),
        other => panic!("expected Section, got {:?}", other),
    }
    assert_eq!(result.root.children[0].children.len(), 1);
    assert!(matches!(
        result.root.children[0].children[0].kind,
        NodeKind::Paragraph { .. }
    ));
}

#[test]
fn three_page_document() {
    let prims = vec![
        hinted(text_prim(0, 0.1, "Page 0", 0), HintKind::Paragraph),
        hinted(text_prim(1, 0.1, "Page 1", 1), HintKind::Paragraph),
        hinted(text_prim(2, 0.1, "Page 2", 2), HintKind::Paragraph),
    ];
    let engine = StructureEngine::new(StructureConfig::default());
    let result = engine.structure(decode_result(prims));

    assert_eq!(result.root.children.len(), 3);
    for (i, child) in result.root.children.iter().enumerate() {
        match &child.kind {
            NodeKind::Paragraph { text } => assert_eq!(text, &format!("Page {}", i)),
            other => panic!("expected Paragraph, got {:?}", other),
        }
    }
}
