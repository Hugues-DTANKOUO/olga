use super::*;
// ---------------------------------------------------------------------------
// Table assembly with mixed hints (the DOCX bug scenario)
// ---------------------------------------------------------------------------

#[test]
fn table_assembles_correctly_with_paragraph_and_table_cell_hints() {
    // Simulates the DOCX pattern: each cell primitive carries both
    // Paragraph and TableCell hints. After the classify() fix, the
    // assembler should produce a proper Table node.
    let mut asm = Assembler::with_defaults();

    let mut prims = Vec::new();
    for (row, col, text) in &[(0u32, 0u32, "A1"), (0, 1, "B1"), (1, 0, "A2"), (1, 1, "B2")] {
        let mut p = text_prim(0.1 + (*row as f32) * 0.05, text, (row * 2 + col) as u64);
        // Paragraph first (like DOCX decoder does before fix B)
        p.hints.push(SemanticHint::from_format(HintKind::Paragraph));
        p.hints
            .push(SemanticHint::from_format(HintKind::ContainedInTableCell {
                row: *row,
                col: *col,
                depth: 0,
            }));
        p.hints.push(SemanticHint::from_format(HintKind::TableCell {
            row: *row,
            col: *col,
            rowspan: 1,
            colspan: 1,
        }));
        prims.push(p);
    }

    asm.process_page(&page_flush(0, prims));
    let result = asm.finish();

    // Should have a Table node, not 4 paragraphs.
    let tables: Vec<_> = result
        .root
        .children
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::Table { .. }))
        .collect();
    assert_eq!(tables.len(), 1, "expected 1 table, got {}", tables.len());

    let table = &tables[0];
    // Table should contain rows with cells
    let cells: Vec<_> = table
        .children
        .iter()
        .flat_map(|row| row.children.iter())
        .filter(|n| matches!(n.kind, NodeKind::TableCell { .. }))
        .collect();
    assert_eq!(cells.len(), 4, "expected 4 cells, got {}", cells.len());
}
