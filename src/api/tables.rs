//! `Table` — tables extracted from the structured document tree.
//!
//! Tables are surfaced directly from the [`crate::structure`] tree: each
//! [`crate::model::NodeKind::Table`] node becomes one [`Table`] entry, with
//! its `TableRow` / `TableCell` descendants flattened into a single cell
//! vector for easy iteration.
//!
//! # Nesting
//!
//! The primitive layer flattens nested tables into the parent via
//! `FlattenedFromTableCell` hints (see [`crate::model::HintKind`]), so by
//! the time tables reach the structure tree they are single-level. A few
//! exotic inputs may still produce nested `Table` nodes below `TableCell`;
//! in that case each outer and inner table is collected as its own
//! independent entry — no ownership tracking — which matches how downstream
//! consumers (indexing, markdown rendering) treat them.

use serde::{Deserialize, Serialize};

use crate::model::{BoundingBox, DocumentNode, NodeKind};

/// One cell inside a [`Table`].
///
/// `row` / `col` are 0-based; `rowspan` / `colspan` are `>= 1`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableCell {
    pub row: u32,
    pub col: u32,
    pub rowspan: u32,
    pub colspan: u32,
    pub text: String,
}

/// A table extracted from the document.
///
/// `rows` / `cols` are the logical dimensions reported by the structure
/// engine (may be larger than `cells.len()` if some cells are empty /
/// merged). `first_page` and `last_page` are **1-based**; they are equal
/// for tables that don't cross page boundaries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Table {
    pub rows: u32,
    pub cols: u32,
    pub first_page: u32,
    pub last_page: u32,
    pub bbox: BoundingBox,
    pub cells: Vec<TableCell>,
}

impl Table {
    /// `true` when the table spans more than one page.
    pub fn is_cross_page(&self) -> bool {
        self.last_page > self.first_page
    }

    /// `true` when this table covers the given 1-based page number.
    pub fn covers_page(&self, page: u32) -> bool {
        self.first_page <= page && page <= self.last_page
    }
}

/// Walk the structure tree, collecting every table in document order.
pub(crate) fn collect_tables(root: &DocumentNode) -> Vec<Table> {
    let mut out = Vec::new();
    walk(root, &mut out);
    out
}

/// Filter a document-level table list to those covering the given 1-based page.
pub(crate) fn tables_on_page(tables: &[Table], page: u32) -> Vec<Table> {
    tables
        .iter()
        .filter(|t| t.covers_page(page))
        .cloned()
        .collect()
}

fn walk(node: &DocumentNode, out: &mut Vec<Table>) {
    if let NodeKind::Table { rows, cols } = &node.kind {
        let mut cells: Vec<TableCell> = Vec::new();
        collect_cells(node, &mut cells);
        // `source_pages` is 0-based, half-open: start..end. Clamp `end` so
        // `last_page` is inclusive on the public API.
        let first_page = node.source_pages.start.saturating_add(1);
        let last_page = node
            .source_pages
            .end
            .saturating_sub(1)
            .saturating_add(1)
            .max(first_page);
        out.push(Table {
            rows: *rows,
            cols: *cols,
            first_page,
            last_page,
            bbox: node.bbox,
            cells,
        });
    }
    for child in &node.children {
        walk(child, out);
    }
}

fn collect_cells(node: &DocumentNode, out: &mut Vec<TableCell>) {
    // We recurse only through `Table` / `TableRow` / `TableCell`. Walking
    // deeper (e.g., into a nested Table sitting under a TableCell) would
    // inflate this table's cell list with cells that belong to a different
    // table. Those nested tables are picked up independently by `walk`.
    match &node.kind {
        NodeKind::TableCell {
            row,
            col,
            rowspan,
            colspan,
            text,
        } => {
            out.push(TableCell {
                row: *row,
                col: *col,
                rowspan: *rowspan,
                colspan: *colspan,
                text: text.clone(),
            });
        }
        NodeKind::Table { .. } | NodeKind::TableRow => {
            for child in &node.children {
                collect_cells(child, out);
            }
        }
        _ => {}
    }
}
