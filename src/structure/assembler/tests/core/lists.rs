use super::*;

#[test]
fn list_from_item_hints() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![
            hinted(
                text_prim(0.1, "first", 0),
                HintKind::ListItem {
                    depth: 0,
                    ordered: false,
                    list_group: None,
                },
            ),
            hinted(
                text_prim(0.15, "second", 1),
                HintKind::ListItem {
                    depth: 0,
                    ordered: false,
                    list_group: None,
                },
            ),
            hinted(
                text_prim(0.2, "third", 2),
                HintKind::ListItem {
                    depth: 0,
                    ordered: false,
                    list_group: None,
                },
            ),
        ],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    assert_eq!(result.root.children.len(), 1);
    let list = &result.root.children[0];
    match &list.kind {
        NodeKind::List { ordered } => assert!(!ordered),
        other => panic!("expected List, got {:?}", other),
    }
    assert_eq!(list.children.len(), 3);
    assert_eq!(list.children[0].text(), Some("first"));
    assert_eq!(list.children[1].text(), Some("second"));
    assert_eq!(list.children[2].text(), Some("third"));
}

#[test]
fn ordered_list() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![hinted(
            text_prim(0.1, "step 1", 0),
            HintKind::ListItem {
                depth: 0,
                ordered: true,
                list_group: None,
            },
        )],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    match &result.root.children[0].kind {
        NodeKind::List { ordered } => assert!(ordered),
        other => panic!("expected ordered List, got {:?}", other),
    }
}

#[test]
fn distinct_list_groups_emit_separate_list_nodes() {
    // Two bullet lists back-to-back, no intervening role. The detector emits
    // distinct `list_group` identifiers → the assembler must split them into
    // two separate `List` nodes instead of merging them into one.
    //
    // Each primitive carries a bullet marker in its text because that's what
    // the real `ListDetector` produces: one hint per primitive within a list
    // box, with the first primitive of each item carrying the marker. The
    // assembler uses `text_starts_list_marker` to decide item boundaries
    // within a single `list_group`; the group change is what splits two
    // adjacent lists.
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![
            hinted(
                text_prim(0.10, "\u{2022} Alpha", 0),
                HintKind::ListItem {
                    depth: 0,
                    ordered: false,
                    list_group: Some(0),
                },
            ),
            hinted(
                text_prim(0.13, "\u{2022} Beta", 1),
                HintKind::ListItem {
                    depth: 0,
                    ordered: false,
                    list_group: Some(0),
                },
            ),
            hinted(
                text_prim(0.20, "\u{2022} Gamma", 2),
                HintKind::ListItem {
                    depth: 0,
                    ordered: false,
                    list_group: Some(1),
                },
            ),
            hinted(
                text_prim(0.23, "\u{2022} Delta", 3),
                HintKind::ListItem {
                    depth: 0,
                    ordered: false,
                    list_group: Some(1),
                },
            ),
        ],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    // Two List nodes, each with two items — the group change between item 2
    // and item 3 forces the split; without it, all four items would land in
    // one list.
    let lists: Vec<_> = result
        .root
        .children
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::List { .. }))
        .collect();
    assert_eq!(
        lists.len(),
        2,
        "expected two List nodes for two distinct groups, got {}: {:#?}",
        lists.len(),
        result.root.children
    );
    assert_eq!(lists[0].children.len(), 2);
    assert_eq!(lists[0].children[0].text(), Some("\u{2022} Alpha"));
    assert_eq!(lists[0].children[1].text(), Some("\u{2022} Beta"));
    assert_eq!(lists[1].children.len(), 2);
    assert_eq!(lists[1].children[0].text(), Some("\u{2022} Gamma"));
    assert_eq!(lists[1].children[1].text(), Some("\u{2022} Delta"));
}

#[test]
fn ordered_to_unordered_transition_splits_list() {
    // A numbered list followed directly by a bulleted list must not be merged
    // into a single List node — the `ordered` flag can only take one value.
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![
            hinted(
                text_prim(0.10, "1. Step one", 0),
                HintKind::ListItem {
                    depth: 0,
                    ordered: true,
                    list_group: Some(0),
                },
            ),
            hinted(
                text_prim(0.13, "2. Step two", 1),
                HintKind::ListItem {
                    depth: 0,
                    ordered: true,
                    list_group: Some(0),
                },
            ),
            hinted(
                text_prim(0.20, "• Bullet a", 2),
                HintKind::ListItem {
                    depth: 0,
                    ordered: false,
                    list_group: Some(1),
                },
            ),
            hinted(
                text_prim(0.23, "• Bullet b", 3),
                HintKind::ListItem {
                    depth: 0,
                    ordered: false,
                    list_group: Some(1),
                },
            ),
        ],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    let lists: Vec<_> = result
        .root
        .children
        .iter()
        .filter_map(|n| match &n.kind {
            NodeKind::List { ordered } => Some((*ordered, n)),
            _ => None,
        })
        .collect();
    assert_eq!(lists.len(), 2, "expected two List nodes, got {:#?}", lists);
    // First list is ordered, second unordered.
    assert!(lists[0].0, "first list should be ordered");
    assert!(!lists[1].0, "second list should be unordered");
    assert_eq!(lists[0].1.children.len(), 2);
    assert_eq!(lists[1].1.children.len(), 2);
}

#[test]
fn same_group_items_stay_merged_even_across_depth_change() {
    // Consecutive items with the SAME list_group but different depths are still
    // part of one List — indentation is a nested-depth signal, not a group
    // break. This also guards against over-aggressive splitting in the new
    // grouping logic.
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![
            hinted(
                text_prim(0.10, "Parent A", 0),
                HintKind::ListItem {
                    depth: 0,
                    ordered: false,
                    list_group: Some(0),
                },
            ),
            hinted(
                text_prim(0.13, "Child A.1", 1),
                HintKind::ListItem {
                    depth: 1,
                    ordered: false,
                    list_group: Some(0),
                },
            ),
            hinted(
                text_prim(0.16, "Parent B", 2),
                HintKind::ListItem {
                    depth: 0,
                    ordered: false,
                    list_group: Some(0),
                },
            ),
        ],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    let lists: Vec<_> = result
        .root
        .children
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::List { .. }))
        .collect();
    assert_eq!(
        lists.len(),
        1,
        "one group_id → one List node, regardless of depth changes; got {:#?}",
        lists
    );
    assert_eq!(lists[0].children.len(), 3);
}

#[test]
fn none_list_group_preserves_legacy_merge() {
    // All items carry list_group = None → the new grouping logic must NOT
    // split the list. This protects every legacy hint source (DOCX/HTML/XLSX
    // decoders and any pre-B2 test fixture) that never populated list_group.
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![
            hinted(
                text_prim(0.10, "one", 0),
                HintKind::ListItem {
                    depth: 0,
                    ordered: false,
                    list_group: None,
                },
            ),
            hinted(
                text_prim(0.13, "two", 1),
                HintKind::ListItem {
                    depth: 0,
                    ordered: false,
                    list_group: None,
                },
            ),
            hinted(
                text_prim(0.16, "three", 2),
                HintKind::ListItem {
                    depth: 0,
                    ordered: false,
                    list_group: None,
                },
            ),
        ],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    let lists: Vec<_> = result
        .root
        .children
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::List { .. }))
        .collect();
    assert_eq!(lists.len(), 1);
    assert_eq!(lists[0].children.len(), 3);
}

#[test]
fn list_confidence_tracks_minimum() {
    let mut asm = Assembler::with_defaults();
    let flush = page_flush(
        0,
        vec![
            hinted_conf(
                text_prim(0.1, "a", 0),
                HintKind::ListItem {
                    depth: 0,
                    ordered: false,
                    list_group: None,
                },
                0.95,
            ),
            hinted_conf(
                text_prim(0.15, "b", 1),
                HintKind::ListItem {
                    depth: 0,
                    ordered: false,
                    list_group: None,
                },
                0.6,
            ),
        ],
    );
    asm.process_page(&flush);
    let result = asm.finish();

    let list = &result.root.children[0];
    assert!(
        (list.confidence - 0.6).abs() < 0.01,
        "list confidence should be min of items: {}",
        list.confidence
    );
}
