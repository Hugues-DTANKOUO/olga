use super::blocks::Block;

fn build_table_grid(table_blocks: &[Block]) -> (Vec<Vec<String>>, usize, usize) {
    let max_row = table_blocks
        .iter()
        .filter_map(|b| b.table_cell.map(|(r, _)| r))
        .max()
        .unwrap_or(0);
    let max_col = table_blocks
        .iter()
        .filter_map(|b| b.table_cell.map(|(_, c)| c))
        .max()
        .unwrap_or(0);

    let n_rows = max_row as usize + 1;
    let n_cols = max_col as usize + 1;

    let mut grid: Vec<Vec<String>> = vec![vec![String::new(); n_cols]; n_rows];
    for block in table_blocks {
        if let Some((r, c)) = block.table_cell {
            let ri = r as usize;
            let ci = c as usize;
            if ri < n_rows && ci < n_cols {
                grid[ri][ci] = block.text.clone();
            }
        }
    }

    (grid, n_rows, n_cols)
}

/// Compute column widths with a minimum of 3.
fn column_widths(grid: &[Vec<String>], n_cols: usize) -> Vec<usize> {
    let mut widths = vec![3usize; n_cols];
    for row in grid {
        for (i, cell) in row.iter().enumerate() {
            if i < n_cols {
                widths[i] = widths[i].max(cell.len());
            }
        }
    }
    widths
}

/// Render table blocks as a GFM Markdown table.
pub(super) fn render_table_gfm(table_blocks: &[Block], out: &mut String) {
    let (grid, n_rows, n_cols) = build_table_grid(table_blocks);
    if n_rows == 0 {
        return;
    }

    // For GFM, apply inline formatting to cells.
    let formatted_grid: Vec<Vec<String>> = grid
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(ci, cell_text)| {
                    // Find the original block to get bold/italic info.
                    let block = table_blocks.iter().find(|b| {
                        b.table_cell
                            .is_some_and(|(_, c)| c as usize == ci && b.text == *cell_text)
                    });
                    match block {
                        Some(b) if b.is_bold => format!("**{}**", cell_text),
                        Some(b) if b.is_italic => format!("*{}*", cell_text),
                        _ => cell_text.clone(),
                    }
                })
                .collect()
        })
        .collect();

    let widths = column_widths(&formatted_grid, n_cols);

    for (i, row) in formatted_grid.iter().enumerate() {
        let mut line = String::from("| ");
        for (j, cell) in row.iter().enumerate() {
            if j > 0 {
                line.push_str(" | ");
            }
            let w = widths.get(j).copied().unwrap_or(3);
            line.push_str(&format!("{:<width$}", cell, width = w));
        }
        line.push_str(" |");
        out.push_str(&line);
        out.push('\n');

        // GFM separator after header row.
        if i == 0 {
            let mut sep = String::from("| ");
            for (j, w) in widths.iter().enumerate() {
                if j > 0 {
                    sep.push_str(" | ");
                }
                sep.push_str(&"-".repeat(*w));
            }
            sep.push_str(" |");
            out.push_str(&sep);
            out.push('\n');
        }
    }
}

/// Render table blocks as aligned ASCII columns (plain text).
pub(super) fn render_table_ascii(table_blocks: &[Block], out: &mut String) {
    let (grid, _n_rows, n_cols) = build_table_grid(table_blocks);
    let widths = column_widths(&grid, n_cols);

    for (i, row) in grid.iter().enumerate() {
        let line: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(j, cell)| {
                if j < widths.len() {
                    format!("{:<width$}", cell, width = widths[j])
                } else {
                    cell.clone()
                }
            })
            .collect();
        out.push_str(&line.join(" | "));
        out.push('\n');

        // Separator after header row.
        if i == 0 {
            let sep: Vec<String> = widths.iter().map(|w| "-".repeat(*w)).collect();
            out.push_str(&sep.join("-+-"));
            out.push('\n');
        }
    }
}
