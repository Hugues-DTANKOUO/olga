//! Internal types used during XLSX sheet processing.

use std::collections::BTreeMap;

/// Represents a merged cell range parsed from `<mergeCells>`.
#[derive(Debug, Clone)]
pub(crate) struct MergedRange {
    /// Top-left row (0-based).
    pub start_row: u32,
    /// Top-left column (0-based).
    pub start_col: u32,
    /// Number of rows spanned.
    pub rowspan: u32,
    /// Number of columns spanned.
    pub colspan: u32,
}

/// Native SpreadsheetML table metadata parsed from `tableParts` / `table*.xml`.
#[derive(Debug, Clone)]
pub(crate) struct NativeTable {
    /// Top-left row of the table ref (0-based).
    pub start_row: u32,
    /// Top-left column of the table ref (0-based).
    pub start_col: u32,
    /// Bottom-right row of the table ref (0-based, inclusive).
    pub end_row: u32,
    /// Bottom-right column of the table ref (0-based, inclusive).
    pub end_col: u32,
    /// Number of header rows declared by the native table.
    pub header_row_count: u32,
}

impl NativeTable {
    /// Check if a cell belongs to the table range.
    pub fn contains(&self, row: u32, col: u32) -> bool {
        row >= self.start_row && row <= self.end_row && col >= self.start_col && col <= self.end_col
    }

    /// Check if a cell falls inside a declared header row.
    pub fn is_header_cell(&self, row: u32, col: u32) -> bool {
        self.contains(row, col)
            && self.header_row_count > 0
            && row < self.start_row.saturating_add(self.header_row_count)
    }
}

/// Native hyperlink range metadata parsed from sheet XML / relationships.
#[derive(Debug, Clone)]
pub(crate) struct SheetHyperlink {
    pub start_row: u32,
    pub start_col: u32,
    pub end_row: u32,
    pub end_col: u32,
    pub target: String,
}

impl SheetHyperlink {
    pub fn contains(&self, row: u32, col: u32) -> bool {
        row >= self.start_row && row <= self.end_row && col >= self.start_col && col <= self.end_col
    }
}

/// Native per-sheet SpreadsheetML metadata used to build a logical grid contract.
#[derive(Debug, Clone, Default)]
pub(crate) struct SheetNativeMetadata {
    /// Merged cell ranges declared by the sheet.
    pub merged: Vec<MergedRange>,
    /// Native table declarations for the sheet.
    pub native_tables: Vec<NativeTable>,
    /// Native hyperlink declarations for the sheet.
    pub hyperlinks: Vec<SheetHyperlink>,
    /// Cell comments keyed by (row, col). Ordered for determinism.
    ///
    /// Merges the two OOXML comment systems at this layer:
    ///
    /// - Legacy notes from `xl/comments{N}.xml` (rel-type
    ///   `…/2006/relationships/comments`).
    /// - Modern threaded comments from `xl/threadedComments/threadedComment{N}.xml`
    ///   (rel-type `.../2017/10/relationships/threadedComment`), joined
    ///   with author resolution via `xl/persons/person{N}.xml`.
    ///
    /// When both reference the same cell, threaded wins (richer and
    /// canonical in modern Excel 365 files).
    pub comments: BTreeMap<(u32, u32), String>,
}

/// Per-sheet processing context.
#[derive(Debug)]
pub(crate) struct SheetContext {
    /// Sheet index (0-based), used as the "page" number in Primitives.
    pub page: u32,
    /// Total number of rows with data (for Y normalization).
    pub total_rows: u32,
    /// Total number of columns with data (for X normalization).
    pub total_cols: u32,
    /// Merged cell ranges for this sheet.
    pub merged: Vec<MergedRange>,
    /// Native SpreadsheetML table declarations.
    pub native_tables: Vec<NativeTable>,
    /// Native hyperlink declarations.
    pub hyperlinks: Vec<SheetHyperlink>,
    /// Cell comments resolved from either legacy or threaded parts.
    pub comments: BTreeMap<(u32, u32), String>,
    /// Visible row count used for logical-grid normalization.
    pub visible_rows: u32,
    /// Visible column count used for logical-grid normalization.
    pub visible_cols: u32,
    visible_row_offsets: Vec<u32>,
    visible_col_offsets: Vec<u32>,
}

impl SheetContext {
    /// Build a sheet context from raw dimensions and parsed SpreadsheetML metadata.
    ///
    /// OOXML `<row hidden="true"/>` and `<col hidden="true"/>` are
    /// *UI-visibility* signals — they control whether Excel's grid hides
    /// the row/column in the spreadsheet view — not authorial signals that
    /// the values should be excluded from extraction. Mainstream readers
    /// (`pandas.read_excel` via openpyxl, `calamine`, Apache POI,
    /// LibreOffice's converters) surface hidden-cell values unconditionally,
    /// because the values remain part of the workbook's data regardless of
    /// whether they are rendered on screen. Treating `hidden="true"` as an
    /// extraction filter silently drops authored content (e.g. a hidden
    /// "GST/QST" column in a budget sheet) that every other extractor emits.
    ///
    /// The sheet context therefore ignores the hidden flags entirely:
    /// positioning and emission treat every row and column as present,
    /// offsets collapse to identity, and `visible_rows` == `total_rows`.
    pub fn new(page: u32, total_rows: u32, total_cols: u32, metadata: SheetNativeMetadata) -> Self {
        let total_rows = total_rows.max(1);
        let total_cols = total_cols.max(1);
        let row_offsets = (0..total_rows).collect();
        let col_offsets = (0..total_cols).collect();

        Self {
            page,
            total_rows,
            total_cols,
            merged: metadata.merged,
            native_tables: metadata.native_tables,
            hyperlinks: metadata.hyperlinks,
            comments: metadata.comments,
            visible_rows: total_rows,
            visible_cols: total_cols,
            visible_row_offsets: row_offsets,
            visible_col_offsets: col_offsets,
        }
    }

    /// Resolved comment text for a cell, if any.
    pub fn comment_for_cell(&self, row: u32, col: u32) -> Option<&str> {
        self.comments.get(&(row, col)).map(|s| s.as_str())
    }

    /// Check if a cell at (row, col) is the top-left origin of a merge.
    /// Returns (rowspan, colspan) if merged, or (1, 1) if not.
    pub fn merge_span(&self, row: u32, col: u32) -> (u32, u32) {
        for m in &self.merged {
            if m.start_row == row && m.start_col == col {
                return (m.rowspan, m.colspan);
            }
        }
        (1, 1)
    }

    /// Check if a cell is inside a merge but NOT the origin — should be skipped.
    pub fn is_merged_non_origin(&self, row: u32, col: u32) -> bool {
        for m in &self.merged {
            let in_range = row >= m.start_row
                && row < m.start_row + m.rowspan
                && col >= m.start_col
                && col < m.start_col + m.colspan;
            if in_range && (row != m.start_row || col != m.start_col) {
                return true;
            }
        }
        false
    }

    /// Logical-grid row position for a cell. Identity since hidden rows
    /// participate in positioning (see [`SheetContext::new`]).
    pub fn visible_row_position(&self, row: u32) -> u32 {
        self.visible_row_offsets[row as usize]
    }

    /// Logical-grid column position for a cell. Identity since hidden columns
    /// participate in positioning (see [`SheetContext::new`]).
    pub fn visible_col_position(&self, col: u32) -> u32 {
        self.visible_col_offsets[col as usize]
    }

    /// Logical-grid row span. Hidden rows count toward the span because
    /// they remain part of the merge geometry.
    pub fn visible_row_span(&self, _start_row: u32, rowspan: u32) -> u32 {
        rowspan.max(1)
    }

    /// Logical-grid column span. Hidden columns count toward the span because
    /// they remain part of the merge geometry.
    pub fn visible_col_span(&self, _start_col: u32, colspan: u32) -> u32 {
        colspan.max(1)
    }

    /// Resolve the native table containing a cell, if any.
    pub fn native_table_for_cell(&self, row: u32, col: u32) -> Option<&NativeTable> {
        self.native_tables
            .iter()
            .find(|table| table.contains(row, col))
    }

    pub fn hyperlink_for_cell(&self, row: u32, col: u32) -> Option<&str> {
        self.hyperlinks
            .iter()
            .find(|link| link.contains(row, col))
            .map(|link| link.target.as_str())
    }
}
