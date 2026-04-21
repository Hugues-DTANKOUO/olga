use super::*;

impl TableDetector {
    /// Extract the set of unique column X-positions from detected table hints.
    pub(super) fn extract_column_positions(hints: &[DetectedHint]) -> Vec<f32> {
        let mut cols: Vec<u32> = Vec::new();
        // We can't get X from the hints directly (they only carry row/col indices).
        // Instead, collect unique column indices — the caller compares column *count*
        // and the assembler's stored X-positions for geometric matching.
        //
        // However, for a more robust comparison we track the actual col indices
        // present. The match is valid when the number of distinct columns is the same.
        for hint in hints {
            let col = match &hint.hint.kind {
                HintKind::TableCell { col, .. } => *col,
                HintKind::TableHeader { col } => *col,
                _ => continue,
            };
            if !cols.contains(&col) {
                cols.push(col);
            }
        }
        cols.sort_unstable();
        // Convert to f32 for the matching API (col indices as proxy).
        cols.into_iter().map(|c| c as f32).collect()
    }

    /// Check whether two sets of column positions match within tolerance.
    ///
    /// Uses column count as the primary signal: if the previous table had N
    /// columns and this page's table also has N columns, it is very likely
    /// a continuation. This is deliberately conservative — column count is a
    /// strong enough signal for bordered and stream tables alike, and avoids
    /// false positives from coincidental X-alignment of unrelated content.
    pub(super) fn columns_match(prev: &[f32], current: &[f32], _tolerance: f32) -> bool {
        if prev.is_empty() || current.is_empty() {
            return false;
        }
        // Primary signal: same number of columns.
        prev.len() == current.len()
    }

    /// Promote `TableCell` and `TableHeader` hints to `TableCellContinuation`.
    ///
    /// Headers on continuation pages are also promoted — the assembler's
    /// repeated-header detection will handle skipping them if needed.
    pub(super) fn promote_to_continuation(hints: Vec<DetectedHint>) -> Vec<DetectedHint> {
        hints
            .into_iter()
            .map(|mut dh| {
                dh.hint.kind = match dh.hint.kind {
                    HintKind::TableCell {
                        row,
                        col,
                        rowspan,
                        colspan,
                    } => HintKind::TableCellContinuation {
                        row,
                        col,
                        rowspan,
                        colspan,
                    },
                    HintKind::TableHeader { col } => HintKind::TableCellContinuation {
                        row: 0,
                        col,
                        rowspan: 1,
                        colspan: 1,
                    },
                    other => other,
                };
                dh
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Y-gap splitting for stream mode
// ---------------------------------------------------------------------------
