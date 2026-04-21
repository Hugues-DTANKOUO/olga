use super::*;

impl TableDetector {
    /// Infer rowspan/colspan for lattice-mode cells.
    ///
    /// Uses two signals:
    /// 1. **Occupancy**: empty neighbor cells in the same row (colspan) or
    ///    column (rowspan) are absorbed into the span.
    /// 2. **Bbox extent**: the text primitive's bounding box reaching into
    ///    adjacent empty cells confirms the span direction. Both colspan
    ///    and rowspan are checked bidirectionally (left/right, up/down).
    pub(super) fn infer_lattice_spans(
        &self,
        assignments: &[(usize, usize, usize)],
        page: &[Primitive],
        table: &cells::Table,
    ) -> Vec<(usize, usize, usize, u32, u32)> {
        let n_rows = table.num_rows();
        let n_cols = table.num_cols();
        let mut occupied: std::collections::HashSet<(usize, usize)> =
            std::collections::HashSet::with_capacity(assignments.len());
        for &(_idx, row, col) in assignments {
            if row < n_rows && col < n_cols {
                occupied.insert((row, col));
            }
        }

        assignments
            .iter()
            .map(|&(idx, row, col)| {
                let prim = &page[idx];
                let text_left = prim.bbox.x;
                let text_right = prim.bbox.right();
                let text_bottom = prim.bbox.bottom();

                let mut col_start = col;
                for c in (0..col).rev() {
                    if !occupied.contains(&(row, c))
                        && let Some(cell_bounds) = table.cell_bounds(row, c)
                        && text_left <= cell_bounds.x1
                    {
                        col_start = c;
                        continue;
                    }
                    break;
                }

                let mut col_end = col;
                for c in (col + 1)..n_cols {
                    if !occupied.contains(&(row, c))
                        && let Some(cell_bounds) = table.cell_bounds(row, c)
                        && text_right >= cell_bounds.x0
                    {
                        col_end = c;
                        continue;
                    }
                    break;
                }

                let colspan = (col_end - col_start + 1) as u32;
                let final_col = col_start;

                let text_top = prim.bbox.y;
                let span_empty_at_row = |r: usize| -> bool {
                    (col_start..=col_end).all(|c| !occupied.contains(&(r, c)))
                };

                let mut row_start = row;
                for r in (0..row).rev() {
                    if span_empty_at_row(r)
                        && let Some(cell_bounds) = table.cell_bounds(r, col)
                        && text_top <= cell_bounds.y1
                    {
                        row_start = r;
                        continue;
                    }
                    break;
                }

                let mut row_end = row;
                for r in (row + 1)..n_rows {
                    if span_empty_at_row(r)
                        && let Some(cell_bounds) = table.cell_bounds(r, col)
                        && text_bottom >= cell_bounds.y0
                    {
                        row_end = r;
                        continue;
                    }
                    break;
                }

                let rowspan = (row_end - row_start + 1) as u32;
                let final_row = row_start;

                (idx, final_row, final_col, rowspan, colspan)
            })
            .collect()
    }
}
