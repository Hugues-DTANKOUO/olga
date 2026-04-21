//! Reconcile snapped rules with the rows already occupied by text
//! (stage 4 of the rules pipeline).
//!
//! Moves horizontals off text rows when a geometric signal tells us which
//! side to nudge toward, and makes sure connected verticals inherit the
//! shift so corners stay attached. No semantic interpretation.

use std::collections::HashMap;

use super::snap::GridRule;

/// Reconcile the snapped rule set with the rows already occupied by text.
///
/// Two geometric constraints are enforced, neither of them semantic:
///
/// 1. A horizontal rule must not share an integer row with a placed word.
///    When it does, the direction to nudge is picked purely from numeric
///    `exact_row` vs `word_exact`: the rule was ABOVE the word in the PDF
///    (`exact_row < word_exact`) → try `row - 1`, otherwise `row + 1`.
///    Exact coincidence — no geometric signal — drops the rule.
///
/// 2. Once a horizontal has been shifted, any vertical whose endpoint
///    geometrically touched that horizontal (i.e. shares the same
///    `exact_row`, within half a cell) must inherit the shift. Otherwise
///    the `|` detaches from the `-` corner and appears to leak one grid
///    row past the border — which is what "Les traits verticaux débordent"
///    looked like on the SAAQ form.
///
/// Verticals are otherwise left untouched: passing a `|` through a text
/// row is the standard ASCII cell-separator convention.
pub(crate) fn adjust_rules(
    rules: Vec<GridRule>,
    word_exact_rows: &HashMap<usize, f32>,
    target_rows: usize,
) -> Vec<GridRule> {
    // Half-cell proximity: two fractional row positions link iff their
    // quantized (rounded) integer row is the same. That is exactly the
    // case when `|a - b| < 0.5 + ε`. A slightly larger slack tolerates
    // floating-point noise at the half-integer boundary without matching
    // unrelated rules.
    const MATCH_EPS: f32 = 0.5 + 1e-3;

    let mut out: Vec<GridRule> = Vec::with_capacity(rules.len());
    // (exact_row, new_row) for every horizontal kept after adjustment.
    let mut h_shifts: Vec<(f32, usize)> = Vec::new();

    for rule in &rules {
        match *rule {
            GridRule::Horizontal {
                row,
                col_start,
                col_end,
                exact_row,
            } => {
                let Some(new_row) =
                    adjust_horizontal_row(row, exact_row, word_exact_rows, target_rows)
                else {
                    continue;
                };
                h_shifts.push((exact_row, new_row));
                out.push(GridRule::Horizontal {
                    row: new_row,
                    col_start,
                    col_end,
                    exact_row,
                });
            }
            v @ GridRule::Vertical { .. } => out.push(v),
        }
    }

    // Second pass: verticals inherit any horizontal shift they were
    // geometrically attached to. Only endpoints that actually moved are
    // written back — leaving row_start/row_end otherwise untouched.
    for rule in out.iter_mut() {
        if let GridRule::Vertical {
            row_start,
            row_end,
            exact_r_lo,
            exact_r_hi,
            ..
        } = rule
        {
            if let Some(new) = find_matching_shift(&h_shifts, *exact_r_lo, MATCH_EPS) {
                *row_start = new;
            }
            if let Some(new) = find_matching_shift(&h_shifts, *exact_r_hi, MATCH_EPS) {
                *row_end = new;
            }
        }
    }

    // A vertical whose endpoints cross (row_start > row_end) or collapse
    // to a single row after the shift carries no visible extent — drop it.
    out.retain(|r| match r {
        GridRule::Vertical {
            row_start, row_end, ..
        } => row_start < row_end,
        _ => true,
    });

    out
}

/// Pick the new integer row for a horizontal rule that collides with a
/// word row. Returns `None` when the rule has nowhere to go without
/// landing on another word row, or when `exact_row == word_exact` exactly
/// — a degenerate tie with no geometric direction to preserve.
fn adjust_horizontal_row(
    row: usize,
    exact_row: f32,
    word_exact_rows: &HashMap<usize, f32>,
    target_rows: usize,
) -> Option<usize> {
    let Some(&word_exact) = word_exact_rows.get(&row) else {
        return Some(row);
    };
    let up = row.checked_sub(1);
    let down = row.checked_add(1).filter(|r| *r < target_rows);
    let (primary, secondary) = if exact_row < word_exact {
        (up, down)
    } else if exact_row > word_exact {
        (down, up)
    } else {
        return None;
    };
    let is_free = |r: &usize| !word_exact_rows.contains_key(r);
    primary
        .filter(is_free)
        .or_else(|| secondary.filter(is_free))
}

fn find_matching_shift(shifts: &[(f32, usize)], exact: f32, eps: f32) -> Option<usize> {
    // Any connected horizontal is fine — horizontals at the same exact_row
    // share the same post-shift row by construction.
    shifts
        .iter()
        .find(|(er, _)| (er - exact).abs() < eps)
        .map(|&(_, r)| r)
}
