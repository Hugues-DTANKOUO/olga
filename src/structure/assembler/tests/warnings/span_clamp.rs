use super::*;
// ---------------------------------------------------------------------------
// Span auto-correction
// ---------------------------------------------------------------------------

#[test]
fn span_auto_corrected_when_exceeding_grid() {
    // Build a 2x2 table where one cell claims colspan=5 and another rowspan=3.
    // Both exceed the actual grid dimensions and should be clamped.
    let mut asm = Assembler::with_defaults();

    let prims = vec![
        hinted(
            text_prim_geo(0.05, 0.05, 0.4, 12.0, "A", 0, 0),
            HintKind::TableCell {
                row: 0,
                col: 0,
                rowspan: 1,
                colspan: 5, // exceeds 2 cols → clamp to 2
            },
        ),
        hinted(
            text_prim_geo(0.50, 0.05, 0.4, 12.0, "B", 0, 1),
            HintKind::TableCell {
                row: 0,
                col: 1,
                rowspan: 3, // exceeds 2 rows → clamp to 2
                colspan: 1,
            },
        ),
        hinted(
            text_prim_geo(0.05, 0.10, 0.4, 12.0, "C", 0, 2),
            HintKind::TableCell {
                row: 1,
                col: 0,
                rowspan: 1,
                colspan: 1,
            },
        ),
        hinted(
            text_prim_geo(0.50, 0.10, 0.4, 12.0, "D", 0, 3),
            HintKind::TableCell {
                row: 1,
                col: 1,
                rowspan: 1,
                colspan: 1,
            },
        ),
    ];

    asm.process_page(&page_flush(0, prims));
    let result = asm.finish();

    // Verify the table was built.
    let table = &result.root.children[0];
    match &table.kind {
        NodeKind::Table { rows, cols } => {
            assert_eq!(*rows, 2);
            assert_eq!(*cols, 2);
        }
        other => panic!("expected Table, got {:?}", other),
    }

    // Find cell (0,0) — colspan should be clamped from 5 to 2.
    let row0 = &table.children[0];
    let cell_a = &row0.children[0];
    match &cell_a.kind {
        NodeKind::TableCell { colspan, .. } => {
            assert_eq!(*colspan, 2, "colspan should be clamped to 2");
        }
        other => panic!("expected TableCell, got {:?}", other),
    }

    // Find cell (0,1) — rowspan should be clamped from 3 to 2.
    let cell_b = &row0.children[1];
    match &cell_b.kind {
        NodeKind::TableCell { rowspan, .. } => {
            assert_eq!(*rowspan, 2, "rowspan should be clamped to 2");
        }
        other => panic!("expected TableCell, got {:?}", other),
    }

    // Verify warnings were emitted.
    let clamp_warnings: Vec<_> = result
        .warnings
        .iter()
        .filter(|w| w.message.contains("Clamped"))
        .collect();
    assert_eq!(
        clamp_warnings.len(),
        2,
        "expected 2 clamp warnings, got: {:?}",
        clamp_warnings
    );
}

#[test]
fn span_not_clamped_when_within_grid() {
    // A 3x3 table where cell (0,0) has colspan=2 — within bounds, no clamping.
    let mut asm = Assembler::with_defaults();

    let prims = vec![
        hinted(
            text_prim_geo(0.05, 0.05, 0.6, 12.0, "Wide", 0, 0),
            HintKind::TableCell {
                row: 0,
                col: 0,
                rowspan: 1,
                colspan: 2, // valid: 2 ≤ 3 - 0
            },
        ),
        hinted(
            text_prim_geo(0.70, 0.05, 0.2, 12.0, "C", 0, 1),
            HintKind::TableCell {
                row: 0,
                col: 2,
                rowspan: 1,
                colspan: 1,
            },
        ),
        hinted(
            text_prim_geo(0.05, 0.10, 0.2, 12.0, "D", 0, 2),
            HintKind::TableCell {
                row: 1,
                col: 0,
                rowspan: 1,
                colspan: 1,
            },
        ),
        hinted(
            text_prim_geo(0.30, 0.10, 0.2, 12.0, "E", 0, 3),
            HintKind::TableCell {
                row: 1,
                col: 1,
                rowspan: 1,
                colspan: 1,
            },
        ),
        hinted(
            text_prim_geo(0.70, 0.10, 0.2, 12.0, "F", 0, 4),
            HintKind::TableCell {
                row: 1,
                col: 2,
                rowspan: 1,
                colspan: 1,
            },
        ),
    ];

    asm.process_page(&page_flush(0, prims));
    let result = asm.finish();

    // No clamp warnings.
    let clamp_warnings: Vec<_> = result
        .warnings
        .iter()
        .filter(|w| w.message.contains("Clamped"))
        .collect();
    assert!(
        clamp_warnings.is_empty(),
        "no clamping expected, got: {:?}",
        clamp_warnings
    );
}

#[test]
fn span_not_clamped_for_single_row_table() {
    // A single-row table with oversized colspan is left untouched because
    // there is no cross-row evidence to determine the true grid width.
    let mut asm = Assembler::with_defaults();

    let prims = vec![hinted(
        text_prim_geo(0.05, 0.05, 0.4, 12.0, "Wide", 0, 0),
        HintKind::TableCell {
            row: 0,
            col: 0,
            rowspan: 1,
            colspan: 10, // looks oversized, but no other rows to compare
        },
    )];

    asm.process_page(&page_flush(0, prims));
    let result = asm.finish();

    let cell = &result.root.children[0].children[0].children[0];
    match &cell.kind {
        NodeKind::TableCell { colspan, .. } => {
            assert_eq!(*colspan, 10, "single-row table should not clamp");
        }
        other => panic!("expected TableCell, got {:?}", other),
    }
    assert!(
        !result
            .warnings
            .iter()
            .any(|w| w.message.contains("Clamped")),
        "no clamping for single-row table"
    );
}

// ---------------------------------------------------------------------------
