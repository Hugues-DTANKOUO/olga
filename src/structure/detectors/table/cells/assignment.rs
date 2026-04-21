use std::collections::HashMap;

use crate::model::{CharPosition, Primitive, PrimitiveKind};

use super::Table;

/// Assign text primitives to table cells.
///
/// Returns `(primitive_index, row, col)` tuples for all text primitives
/// that fall within a table cell.
///
/// **Character-level assignment**: when a text primitive provides
/// per-character bounding boxes (`char_positions`), each character's
/// midpoint is tested against the table grid. The primitive is assigned
/// to the cell that contains the majority of its characters. This
/// handles the edge case where a single text run straddles two cells —
/// the majority vote picks the correct one.
///
/// **Fallback**: when `char_positions` is absent, the primitive's
/// bounding-box midpoint is used (the original behavior).
pub fn assign_text_to_table(table: &Table, page: &[Primitive]) -> Vec<(usize, usize, usize)> {
    let mut assignments = Vec::new();

    for (idx, prim) in page.iter().enumerate() {
        if let PrimitiveKind::Text {
            ref content,
            ref char_positions,
            ..
        } = prim.kind
        {
            if content.trim().is_empty() {
                continue;
            }

            if let Some(chars) = char_positions
                && !chars.is_empty()
            {
                if let Some((row, col)) = majority_cell(table, chars) {
                    assignments.push((idx, row, col));
                }
                continue;
            }

            let cx = prim.bbox.x + prim.bbox.width / 2.0;
            let cy = prim.bbox.y + prim.bbox.height / 2.0;
            if let Some((row, col)) = table.cell_at(cx, cy) {
                assignments.push((idx, row, col));
            }
        }
    }

    assignments
}

fn majority_cell(table: &Table, chars: &[CharPosition]) -> Option<(usize, usize)> {
    let mut votes: HashMap<(usize, usize), usize> = HashMap::new();

    for cp in chars {
        let h_mid = cp.bbox.x + cp.bbox.width / 2.0;
        let v_mid = cp.bbox.y + cp.bbox.height / 2.0;
        if let Some(cell) = table.cell_at(h_mid, v_mid) {
            *votes.entry(cell).or_default() += 1;
        }
    }

    votes
        .into_iter()
        .max_by_key(|&(_, count)| count)
        .map(|(cell, _)| cell)
}
