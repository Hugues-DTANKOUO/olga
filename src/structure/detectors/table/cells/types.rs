/// A rectangular cell defined by 4 intersection corners.
#[derive(Debug, Clone)]
pub struct Cell {
    /// Top-left corner X.
    pub x0: f32,
    /// Top-left corner Y.
    pub y0: f32,
    /// Bottom-right corner X.
    pub x1: f32,
    /// Bottom-right corner Y.
    pub y1: f32,
}

impl Cell {
    /// Width of the cell.
    #[inline]
    pub fn width(&self) -> f32 {
        self.x1 - self.x0
    }

    /// Height of the cell.
    #[inline]
    pub fn height(&self) -> f32 {
        self.y1 - self.y0
    }
}

/// A detected table: a grid of cells with row/column structure.
#[derive(Debug, Clone)]
pub struct Table {
    /// Cells in the table, stored as a flat list.
    pub cells: Vec<Cell>,
    /// Sorted unique Y positions of row boundaries (top edges of rows + bottom of last row).
    pub row_ys: Vec<f32>,
    /// Sorted unique X positions of column boundaries (left edges of cols + right of last col).
    pub col_xs: Vec<f32>,
}

impl Table {
    /// Number of rows in the table.
    pub fn num_rows(&self) -> usize {
        self.row_ys.len().saturating_sub(1)
    }

    /// Number of columns in the table.
    pub fn num_cols(&self) -> usize {
        self.col_xs.len().saturating_sub(1)
    }

    /// Bounding box of the entire table: (x0, y0, x1, y1).
    pub fn bbox(&self) -> (f32, f32, f32, f32) {
        let x0 = self.col_xs.first().copied().unwrap_or(0.0);
        let y0 = self.row_ys.first().copied().unwrap_or(0.0);
        let x1 = self.col_xs.last().copied().unwrap_or(0.0);
        let y1 = self.row_ys.last().copied().unwrap_or(0.0);
        (x0, y0, x1, y1)
    }

    /// Find the (row, col) of the cell that contains a given point.
    /// Returns `None` if the point is outside the table.
    pub fn cell_at(&self, x: f32, y: f32) -> Option<(usize, usize)> {
        let col = self.col_xs.windows(2).position(|w| x >= w[0] && x < w[1]);
        let row = self.row_ys.windows(2).position(|w| y >= w[0] && y < w[1]);
        match (row, col) {
            (Some(r), Some(c)) => Some((r, c)),
            _ => None,
        }
    }

    /// Get the cell boundaries for a given (row, col).
    pub fn cell_bounds(&self, row: usize, col: usize) -> Option<Cell> {
        if row + 1 >= self.row_ys.len() || col + 1 >= self.col_xs.len() {
            return None;
        }
        Some(Cell {
            x0: self.col_xs[col],
            y0: self.row_ys[row],
            x1: self.col_xs[col + 1],
            y1: self.row_ys[row + 1],
        })
    }
}
