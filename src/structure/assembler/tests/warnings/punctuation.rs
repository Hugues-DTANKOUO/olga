use super::*;
// Geometry merge — sentence-ending punctuation blocking
// ---------------------------------------------------------------------------

#[test]
fn geometry_merge_blocked_by_sentence_ending_period() {
    // "End of first." ends with a long word + period → geometry merge blocked.
    // Even though geometry matches perfectly, the period signals paragraph end.
    let mut asm = Assembler::with_defaults();

    asm.process_page(&page_flush(
        0,
        vec![text_prim_geo(0.05, 0.80, 0.9, 12.0, "End of first.", 0, 0)],
    ));

    asm.process_page(&page_flush(
        1,
        vec![text_prim_geo(
            0.05,
            0.05,
            0.9,
            12.0,
            "Start of second.",
            1,
            1,
        )],
    ));

    let result = asm.finish();
    assert_eq!(
        result.root.children.len(),
        2,
        "sentence-ending period should block geometry merge"
    );
}

#[test]
fn geometry_merge_allowed_after_abbreviation() {
    // "held by Dr." ends with a short word (≤ 4 chars) + period → abbreviation,
    // geometry merge is allowed.
    let mut asm = Assembler::with_defaults();

    asm.process_page(&page_flush(
        0,
        vec![text_prim_geo(0.05, 0.80, 0.9, 12.0, "held by Dr.", 0, 0)],
    ));

    asm.process_page(&page_flush(
        1,
        vec![text_prim_geo(
            0.05,
            0.05,
            0.9,
            12.0,
            "Smith at the event.",
            1,
            1,
        )],
    ));

    let result = asm.finish();
    assert_eq!(
        result.root.children.len(),
        1,
        "abbreviation should allow geometry merge"
    );
    match &result.root.children[0].kind {
        NodeKind::Paragraph { text } => {
            assert!(text.contains("Dr."));
            assert!(text.contains("Smith"));
        }
        other => panic!("expected Paragraph, got {:?}", other),
    }
}

#[test]
fn geometry_merge_blocked_by_exclamation() {
    // "Amazing!" ends with ! → geometry merge blocked regardless.
    let mut asm = Assembler::with_defaults();

    asm.process_page(&page_flush(
        0,
        vec![text_prim_geo(0.05, 0.80, 0.9, 12.0, "Amazing!", 0, 0)],
    ));

    asm.process_page(&page_flush(
        1,
        vec![text_prim_geo(
            0.05,
            0.05,
            0.9,
            12.0,
            "Next topic here.",
            1,
            1,
        )],
    ));

    let result = asm.finish();
    assert_eq!(
        result.root.children.len(),
        2,
        "exclamation mark should block geometry merge"
    );
}
