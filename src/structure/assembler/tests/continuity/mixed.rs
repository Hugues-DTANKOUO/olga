use super::*;
// ---------------------------------------------------------------------------
// Mixed document with cross-page structures
// ---------------------------------------------------------------------------

#[test]
fn mixed_document_heading_para_table_two_pages_para() {
    let mut asm = Assembler::with_defaults();

    // Page 0: heading → paragraph → start of table.
    asm.process_page(&page_flush(
        0,
        vec![
            hinted(text_prim(0.05, "Title", 0), HintKind::Heading { level: 1 }),
            hinted(text_prim(0.1, "Introduction.", 1), HintKind::Paragraph),
            hinted(
                text_prim(0.3, "Col1", 2),
                HintKind::TableCell {
                    row: 0,
                    col: 0,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
            hinted(
                text_prim(0.3, "Col2", 3),
                HintKind::TableCell {
                    row: 0,
                    col: 1,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
        ],
    ));

    // Page 1: table continues → then paragraph.
    asm.process_page(&page_flush(
        1,
        vec![
            hinted(
                text_prim(0.1, "D1", 4),
                HintKind::TableCell {
                    row: 1,
                    col: 0,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
            hinted(
                text_prim(0.1, "D2", 5),
                HintKind::TableCell {
                    row: 1,
                    col: 1,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
            hinted(text_prim(0.5, "Conclusion.", 6), HintKind::Paragraph),
        ],
    ));

    let result = asm.finish();

    // heading, paragraph, table (2 pages), paragraph
    assert_eq!(result.root.children.len(), 4);
    assert!(matches!(
        result.root.children[0].kind,
        NodeKind::Heading { level: 1, .. }
    ));
    assert!(matches!(
        result.root.children[1].kind,
        NodeKind::Paragraph { .. }
    ));
    match &result.root.children[2].kind {
        NodeKind::Table { rows, cols } => {
            assert_eq!(*rows, 2);
            assert_eq!(*cols, 2);
        }
        other => panic!("expected Table, got {:?}", other),
    }
    assert_eq!(result.root.children[2].source_pages, 0..2);
    assert!(matches!(
        result.root.children[3].kind,
        NodeKind::Paragraph { .. }
    ));
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn single_page_document_no_continuity_logic() {
    // Verify that single-page documents work exactly as before.
    let mut asm = Assembler::with_defaults();
    asm.process_page(&page_flush(
        0,
        vec![
            hinted(text_prim(0.1, "Hello", 0), HintKind::Paragraph),
            hinted(
                text_prim(0.3, "Cell", 1),
                HintKind::TableCell {
                    row: 0,
                    col: 0,
                    rowspan: 1,
                    colspan: 1,
                },
            ),
        ],
    ));
    let result = asm.finish();

    assert_eq!(result.root.children.len(), 2);
}

#[test]
fn section_spans_pages_with_continuity() {
    let mut asm = Assembler::with_defaults();

    // Section opens on page 0, continues through pages 1-4.
    asm.process_page(&page_flush(
        0,
        vec![
            hinted(
                text_prim(0.05, "", 0),
                HintKind::SectionStart { title: None },
            ),
            hinted(text_prim(0.1, "p0", 1), HintKind::Paragraph),
        ],
    ));
    for p in 1..5 {
        asm.process_page(&page_flush(
            p,
            vec![hinted(
                text_prim(0.1, &format!("p{}", p), p as u64 + 1),
                HintKind::Paragraph,
            )],
        ));
    }
    asm.process_page(&page_flush(
        5,
        vec![hinted(text_prim(0.1, "", 6), HintKind::SectionEnd)],
    ));

    let result = asm.finish();

    assert_eq!(result.root.children.len(), 1);
    let section = &result.root.children[0];
    assert!(matches!(section.kind, NodeKind::Section { .. }));
    assert_eq!(section.children.len(), 5); // p0..p4
    assert!(section.source_pages.end >= 5);
}

#[test]
fn is_continuation_text_detects_lowercase() {
    assert!(Assembler::is_continuation_text("continued here"));
    assert!(!Assembler::is_continuation_text("New sentence"));
    assert!(!Assembler::is_continuation_text(""));
}

#[test]
fn is_continuation_text_detects_punctuation() {
    assert!(Assembler::is_continuation_text(", and more"));
    assert!(Assembler::is_continuation_text("; also"));
    assert!(Assembler::is_continuation_text(". End."));
    assert!(Assembler::is_continuation_text(") closing"));
    assert!(Assembler::is_continuation_text("\u{2014}dash")); // em-dash
    assert!(Assembler::is_continuation_text("\u{2026}ellipsis")); // …
}
