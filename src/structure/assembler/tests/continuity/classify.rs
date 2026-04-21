use super::*;
// ---------------------------------------------------------------------------
// classify() priority system
// ---------------------------------------------------------------------------

#[test]
fn classify_prefers_table_cell_over_paragraph() {
    // Simulates DOCX pattern: <w:p> inside <w:tc> where Paragraph hint
    // is emitted before TableCell hint.
    let mut p = text_prim(0.1, "cell text", 0);
    p.hints.push(SemanticHint::from_format(HintKind::Paragraph));
    p.hints
        .push(SemanticHint::from_format(HintKind::ContainedInTableCell {
            row: 1,
            col: 2,
            depth: 0,
        }));
    p.hints.push(SemanticHint::from_format(HintKind::TableCell {
        row: 1,
        col: 2,
        rowspan: 1,
        colspan: 1,
    }));

    let role = Assembler::classify(&p);
    assert!(
        matches!(role, PrimaryRole::TableCell { row: 1, col: 2, .. }),
        "expected TableCell, got {:?}",
        role
    );
}

#[test]
fn classify_prefers_list_item_over_paragraph() {
    let mut p = text_prim(0.1, "bullet text", 0);
    p.hints.push(SemanticHint::from_format(HintKind::Paragraph));
    p.hints.push(SemanticHint::from_format(HintKind::ListItem {
        depth: 0,
        ordered: false,
        list_group: None,
    }));

    let role = Assembler::classify(&p);
    assert!(
        matches!(role, PrimaryRole::ListItem { depth: 0, .. }),
        "expected ListItem, got {:?}",
        role
    );
}

#[test]
fn classify_prefers_table_cell_over_heading() {
    // A heading inside a table cell: the structural container wins.
    let mut p = text_prim(0.1, "header text", 0);
    p.hints
        .push(SemanticHint::from_format(HintKind::Heading { level: 2 }));
    p.hints.push(SemanticHint::from_format(HintKind::TableCell {
        row: 0,
        col: 0,
        rowspan: 1,
        colspan: 1,
    }));

    let role = Assembler::classify(&p);
    assert!(
        matches!(role, PrimaryRole::TableCell { row: 0, col: 0, .. }),
        "expected TableCell, got {:?}",
        role
    );
}

#[test]
fn classify_bare_text_falls_through() {
    let p = text_prim(0.1, "bare text", 0);
    let role = Assembler::classify(&p);
    assert!(matches!(role, PrimaryRole::TextBare));
}

#[test]
fn classify_bare_image_falls_through() {
    let p = image_prim(0.1, Some("alt"), 0);
    let role = Assembler::classify(&p);
    assert!(matches!(role, PrimaryRole::ImageBare));
}

#[test]
fn classify_sidebar_loses_to_table_cell() {
    let mut p = text_prim(0.1, "sidebar in table", 0);
    p.hints.push(SemanticHint::from_format(HintKind::Sidebar));
    p.hints.push(SemanticHint::from_format(HintKind::TableCell {
        row: 0,
        col: 0,
        rowspan: 1,
        colspan: 1,
    }));

    let role = Assembler::classify(&p);
    assert!(
        matches!(role, PrimaryRole::TableCell { .. }),
        "expected TableCell, got {:?}",
        role
    );
}
