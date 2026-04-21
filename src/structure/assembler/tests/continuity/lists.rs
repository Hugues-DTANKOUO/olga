use super::*;
// ---------------------------------------------------------------------------
// Cross-page list continuity
// ---------------------------------------------------------------------------

#[test]
fn list_survives_page_boundary_with_hints() {
    let mut asm = Assembler::with_defaults();

    asm.process_page(&page_flush(
        0,
        vec![
            hinted(
                text_prim(0.1, "Item A", 0),
                HintKind::ListItem {
                    depth: 0,
                    ordered: true,
                    list_group: None,
                },
            ),
            hinted(
                text_prim(0.2, "Item B", 1),
                HintKind::ListItem {
                    depth: 0,
                    ordered: true,
                    list_group: None,
                },
            ),
        ],
    ));

    asm.process_page(&page_flush(
        1,
        vec![hinted(
            text_prim(0.1, "Item C", 2),
            HintKind::ListItem {
                depth: 0,
                ordered: true,
                list_group: None,
            },
        )],
    ));

    let result = asm.finish();

    // Single list spanning 2 pages.
    assert_eq!(result.root.children.len(), 1);
    match &result.root.children[0].kind {
        NodeKind::List { ordered } => assert!(*ordered),
        other => panic!("expected List, got {:?}", other),
    }
    assert_eq!(result.root.children[0].children.len(), 3);
    assert_eq!(result.root.children[0].source_pages, 0..2);
}

#[test]
fn list_closes_when_next_page_has_no_list() {
    let mut asm = Assembler::with_defaults();

    asm.process_page(&page_flush(
        0,
        vec![hinted(
            text_prim(0.1, "Item", 0),
            HintKind::ListItem {
                depth: 0,
                ordered: false,
                list_group: None,
            },
        )],
    ));

    asm.process_page(&page_flush(
        1,
        vec![hinted(
            text_prim(0.1, "Normal text", 1),
            HintKind::Paragraph,
        )],
    ));

    let result = asm.finish();

    assert_eq!(result.root.children.len(), 2);
    assert!(matches!(
        result.root.children[0].kind,
        NodeKind::List { .. }
    ));
    assert!(matches!(
        result.root.children[1].kind,
        NodeKind::Paragraph { .. }
    ));
}
