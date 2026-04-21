use super::*;
// ---------------------------------------------------------------------------
// List depth jump at page boundary emits warning
// ---------------------------------------------------------------------------

#[test]
fn list_depth_jump_at_page_boundary_warns() {
    let mut asm = Assembler::with_defaults();

    // Page 0: list at depth 0
    let prims_p0 = vec![hinted(
        text_prim(0.1, "item at depth 0", 0),
        HintKind::ListItem {
            depth: 0,
            ordered: false,
            list_group: None,
        },
    )];
    asm.process_page(&page_flush(0, prims_p0));

    // Page 1: list jumps to depth 3 (suspicious)
    let prims_p1 = vec![hinted(
        text_prim(0.1, "item at depth 3", 1),
        HintKind::ListItem {
            depth: 3,
            ordered: false,
            list_group: None,
        },
    )];
    asm.process_page(&page_flush(1, prims_p1));

    let result = asm.finish();

    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.message.contains("List depth jumped")),
        "expected list depth jump warning, got: {:?}",
        result.warnings
    );
}

#[test]
fn list_depth_increment_by_one_no_warning() {
    let mut asm = Assembler::with_defaults();

    // Page 0: list at depth 0
    let prims_p0 = vec![hinted(
        text_prim(0.1, "item depth 0", 0),
        HintKind::ListItem {
            depth: 0,
            ordered: false,
            list_group: None,
        },
    )];
    asm.process_page(&page_flush(0, prims_p0));

    // Page 1: list at depth 1 (normal nesting)
    let prims_p1 = vec![hinted(
        text_prim(0.1, "item depth 1", 1),
        HintKind::ListItem {
            depth: 1,
            ordered: false,
            list_group: None,
        },
    )];
    asm.process_page(&page_flush(1, prims_p1));

    let result = asm.finish();

    assert!(
        !result
            .warnings
            .iter()
            .any(|w| w.message.contains("List depth jumped")),
        "depth increment of 1 should not warn, got: {:?}",
        result.warnings
    );
}

// ---------------------------------------------------------------------------
// Geometry-aware paragraph merging
// ---------------------------------------------------------------------------

#[test]
fn paragraph_merges_across_page_when_geometry_matches() {
    // Page 0: bare text at x=0.05, width=0.9, font_size=12.
    // Page 1: bare text starting with uppercase "Paris" — same geometry.
    // Without geometry matching this would NOT merge (uppercase start).
    // With geometry matching it SHOULD merge (same column, same style).
    let mut asm = Assembler::with_defaults();

    asm.process_page(&page_flush(
        0,
        vec![text_prim_geo(
            0.05,
            0.80,
            0.9,
            12.0,
            "The conference was held in",
            0,
            0,
        )],
    ));

    asm.process_page(&page_flush(
        1,
        vec![text_prim_geo(
            0.05,
            0.05,
            0.9,
            12.0,
            "Paris last summer.",
            1,
            1,
        )],
    ));

    let result = asm.finish();
    assert_eq!(
        result.root.children.len(),
        1,
        "should merge into one paragraph"
    );
    match &result.root.children[0].kind {
        NodeKind::Paragraph { text } => {
            assert!(text.contains("held in"));
            assert!(text.contains("Paris"));
        }
        other => panic!("expected Paragraph, got {:?}", other),
    }
    assert_eq!(result.root.children[0].source_pages, 0..2);
}

#[test]
fn paragraph_does_not_merge_when_different_column() {
    // Page 0: text at x=0.05 (left column).
    // Page 1: text at x=0.55 (right column) — different column, don't merge.
    let mut asm = Assembler::with_defaults();

    asm.process_page(&page_flush(
        0,
        vec![text_prim_geo(
            0.05,
            0.80,
            0.4,
            12.0,
            "Left column text",
            0,
            0,
        )],
    ));

    asm.process_page(&page_flush(
        1,
        vec![text_prim_geo(
            0.55,
            0.05,
            0.4,
            12.0,
            "Right column text",
            1,
            1,
        )],
    ));

    let result = asm.finish();
    assert_eq!(
        result.root.children.len(),
        2,
        "different X position should produce two paragraphs"
    );
}

#[test]
fn paragraph_does_not_merge_when_different_font_size() {
    // Same column position but different font size (12 vs 18) — don't merge.
    // Different font size signals a heading or different paragraph style.
    let mut asm = Assembler::with_defaults();

    asm.process_page(&page_flush(
        0,
        vec![text_prim_geo(
            0.05,
            0.80,
            0.9,
            12.0,
            "Body text ending here",
            0,
            0,
        )],
    ));

    asm.process_page(&page_flush(
        1,
        vec![text_prim_geo(
            0.05,
            0.05,
            0.9,
            18.0,
            "New Section Title",
            1,
            1,
        )],
    ));

    let result = asm.finish();
    assert_eq!(
        result.root.children.len(),
        2,
        "different font size should produce two paragraphs"
    );
}

#[test]
fn paragraph_does_not_merge_when_different_width() {
    // Same X and font size but very different width (0.9 vs 0.3) — don't merge.
    // Width difference > 10% suggests different column layout.
    let mut asm = Assembler::with_defaults();

    asm.process_page(&page_flush(
        0,
        vec![text_prim_geo(
            0.05,
            0.80,
            0.9,
            12.0,
            "Full-width paragraph",
            0,
            0,
        )],
    ));

    asm.process_page(&page_flush(
        1,
        vec![text_prim_geo(0.05, 0.05, 0.3, 12.0, "Narrow column", 1, 1)],
    ));

    let result = asm.finish();
    assert_eq!(
        result.root.children.len(),
        2,
        "very different width should produce two paragraphs"
    );
}
