//! Resolve same-row column collisions by shifting words right.
//!
//! # Why
//!
//! The grid renderer places each word independently at
//! `round(nx * last_col)`. When two words on the same row round to
//! overlapping (or back-to-back) column intervals, the naive writer
//! simply overwrites any existing cell — silently deleting characters.
//!
//! Three concrete failures on real documents, all the same bug:
//!
//! - A right-aligned `Page 2` and a left-aligned `investors@…com` land
//!   on the same row; `Page 2` overwrites the middle of the email,
//!   producing `investors@stratagem-Page 2ics.com` with `analyt` lost.
//! - A justified `if` (col N..N+2) and `payment` (col N+2..) round to
//!   touching intervals, erasing the separating space → `ifpayment`.
//! - Two table-header labels whose glyphs slightly overlap in PDF
//!   geometry (`Due` and `Montant`) interleave on the grid, giving
//!   `DuMeontant` — one letter overwritten, another misplaced.
//!
//! All three are the same defect: quantized column intervals can
//! collide, and the writer never notices.
//!
//! # What this module guarantees
//!
//! Given the intended `(row, col, length)` of every placed word, return
//! a revised column for each word such that on every row:
//!
//! 1. No two words' column intervals overlap — so no character can ever
//!    be overwritten by a later word.
//! 2. A one-column blank gap separates every consecutive pair — so two
//!    words that were separate in the PDF stay visually separate in the
//!    output, even when rounding would merge them.
//! 3. The relative order within a row is preserved (stable sort on
//!    intended column + input index).
//! 4. A word is never moved LEFT of its intended column — natural PDF
//!    whitespace is preserved wherever it already exists.
//!
//! When invariants 1 and 2 force a word rightward, the grid caller must
//! be ready to grow the row past `target_cols`. A long line is always a
//! better outcome than silent content loss: the original renderer had
//! the opposite tradeoff and that is precisely what broke these three
//! cases.

use std::collections::BTreeMap;

/// Produce a per-item actual column that enforces the no-overlap + 1-gap
/// invariants described at the module level.
///
/// `rows[i]`, `intended_cols[i]`, and `lens[i]` describe item `i`:
/// - `rows[i]` — integer grid row the item was assigned to,
/// - `intended_cols[i]` — column the item wanted before collision check,
/// - `lens[i]` — number of cells the item will occupy when drawn.
///
/// The three slices must have identical length; this is asserted.
pub(crate) fn resolve_cols(rows: &[usize], intended_cols: &[usize], lens: &[usize]) -> Vec<usize> {
    assert_eq!(rows.len(), intended_cols.len());
    assert_eq!(rows.len(), lens.len());
    let n = rows.len();

    // Bucket indices by row so each row is swept independently.
    let mut by_row: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, &row) in rows.iter().enumerate().take(n) {
        by_row.entry(row).or_default().push(i);
    }

    let mut resolved = intended_cols.to_vec();
    for mut idxs in by_row.into_values() {
        // Stable tie-break on input index preserves PDF reading order
        // when two items target the same intended column.
        idxs.sort_by_key(|&i| (intended_cols[i], i));

        // `last_end` is the exclusive-end column of the previous placed
        // item (first cell AFTER the item). Enforcing
        // `start >= last_end + 1` guarantees a one-column blank gap.
        let mut last_end: Option<usize> = None;
        for i in idxs {
            let intended = intended_cols[i];
            let min_start = match last_end {
                None => intended,
                Some(e) => intended.max(e + 1),
            };
            resolved[i] = min_start;
            last_end = Some(min_start + lens[i]);
        }
    }

    resolved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_returns_empty() {
        assert_eq!(resolve_cols(&[], &[], &[]), Vec::<usize>::new());
    }

    #[test]
    fn single_item_returns_intended() {
        assert_eq!(resolve_cols(&[0], &[10], &[5]), vec![10]);
    }

    #[test]
    fn disjoint_rows_never_interact() {
        // Two items on different rows at the same intended column stay
        // at that column — row independence is absolute.
        let got = resolve_cols(&[0, 1], &[5, 5], &[3, 3]);
        assert_eq!(got, vec![5, 5]);
    }

    #[test]
    fn comfortable_gap_same_row_untouched() {
        // "hello" (col 0, len 5 → ends exclusive 5), "world" intended at
        // col 6 — already one clear col gap, nothing to shift.
        let got = resolve_cols(&[0, 0], &[0, 6], &[5, 5]);
        assert_eq!(got, vec![0, 6]);
    }

    #[test]
    fn adjacent_words_get_a_forced_space_gap() {
        // Reproduces the "if payment" → "ifpayment" bug on contract.pdf.
        // "if" at col 3 (len 2 → ends 5). "payment" intended at col 5
        // (touching) must shift to col 6.
        let got = resolve_cols(&[0, 0], &[3, 5], &[2, 7]);
        assert_eq!(got, vec![3, 6]);
    }

    #[test]
    fn overlapping_words_push_right() {
        // Reproduces the "Due" + "Montant" → "DuMeontant" interleave.
        // "Due" at col 0 (len 3 → ends 3), "Montant" intended at col 2
        // (overlapping by 1 col) must land at col 4.
        let got = resolve_cols(&[0, 0], &[0, 2], &[3, 7]);
        assert_eq!(got, vec![0, 4]);
    }

    #[test]
    fn long_word_cascades_through_later_items() {
        // Reproduces the "Page 2" overwriting email on earnings.pdf.
        // email at col 10 (len 33 → ends 43). `Page` intended at col 25,
        // cascades to 44; `2` intended at col 35 cascades to 49.
        let got = resolve_cols(&[0, 0, 0], &[10, 25, 35], &[33, 4, 1]);
        assert_eq!(got, vec![10, 44, 49]);
    }

    #[test]
    fn identical_intended_cols_preserve_input_order() {
        // Two items on the same row with identical intended col: the
        // one appearing first in input stays first and the second
        // slides right by its length + 1.
        let got = resolve_cols(&[0, 0], &[5, 5], &[2, 2]);
        assert_eq!(got, vec![5, 8]);
    }

    #[test]
    fn never_move_left_when_natural_gap_already_large() {
        // A genuinely large natural gap is preserved verbatim — the
        // algorithm never compacts layout.
        let got = resolve_cols(&[0, 0], &[0, 20], &[3, 5]);
        assert_eq!(got, vec![0, 20]);
    }

    #[test]
    fn multi_row_collisions_are_independent() {
        // Row 0 has a collision, row 1 is untouched.
        let got = resolve_cols(&[0, 0, 1], &[0, 2, 0], &[3, 5, 5]);
        assert_eq!(got, vec![0, 4, 0]);
    }

    #[test]
    fn cascade_of_three_on_one_row() {
        // Three back-to-back items all get shifted: 0+3 → 1 shifts to 4
        // (len 3 → ends 7) → 2 shifts to 8.
        let got = resolve_cols(&[0, 0, 0], &[0, 2, 5], &[3, 3, 3]);
        assert_eq!(got, vec![0, 4, 8]);
    }
}
