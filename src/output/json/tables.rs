use serde_json::{Value, json};

use crate::model::{BoundingBox, DocumentNode, NodeKind};

use super::helpers::{round2, round4};
use super::nodes::ElementCounter;

pub(super) struct TableContext {
    pub(super) id: String,
    pub(super) bbox: Value,
    pub(super) page: u32,
    pub(super) source_str: String,
}

pub(super) fn render_table(
    node: &DocumentNode,
    rows: u32,
    cols: u32,
    ctx: TableContext,
    counter: &mut ElementCounter,
) -> Value {
    // Collect all cells from the tree: Table → TableRow → TableCell
    let mut cell_list: Vec<CellInfo> = Vec::new();
    for row_node in &node.children {
        match &row_node.kind {
            NodeKind::TableRow => {
                for cell_node in &row_node.children {
                    if let NodeKind::TableCell {
                        row,
                        col,
                        rowspan,
                        colspan,
                        text,
                    } = &cell_node.kind
                    {
                        cell_list.push(CellInfo {
                            row: *row,
                            col: *col,
                            rowspan: *rowspan,
                            colspan: *colspan,
                            text: text.clone(),
                            bbox: cell_node.bbox,
                            is_header: *row == 0,
                        });
                    }
                }
            }
            NodeKind::TableCell {
                row,
                col,
                rowspan,
                colspan,
                text,
            } => {
                // Direct child cells (no TableRow wrapper)
                cell_list.push(CellInfo {
                    row: *row,
                    col: *col,
                    rowspan: *rowspan,
                    colspan: *colspan,
                    text: text.clone(),
                    bbox: row_node.bbox,
                    is_header: *row == 0,
                });
            }
            _ => {}
        }
    }

    // Build the simple 2D data array.
    let mut grid: Vec<Vec<String>> = vec![vec![String::new(); cols as usize]; rows as usize];
    for cell in &cell_list {
        let r = cell.row as usize;
        let c = cell.col as usize;
        if r < grid.len() && c < grid[0].len() {
            grid[r][c] = cell.text.clone();
        }
    }

    // Separate headers (row 0) from data (rest).
    let (headers, data) = if !grid.is_empty() {
        let headers = grid[0].clone();
        let data: Vec<Vec<String>> = grid.into_iter().skip(1).collect();
        (headers, data)
    } else {
        (Vec::new(), Vec::new())
    };

    // Only emit the detailed cells array when at least one cell has
    // non-trivial spans (rowspan>1 or colspan>1), since headers+data
    // already carry all text content for simple grids.
    let has_complex_spans = cell_list.iter().any(|c| c.rowspan > 1 || c.colspan > 1);

    let mut table_obj = json!({
        "id": ctx.id,
        "type": "table",
        "rows": rows,
        "cols": cols,
        "bbox": ctx.bbox,
        "page": ctx.page,
        "confidence": round2(node.confidence),
        "source": ctx.source_str,
        "headers": headers,
        "data": data,
    });

    if has_complex_spans {
        let cells_json: Vec<Value> = cell_list
            .iter()
            .map(|c| {
                let cell_id = counter.next_id(ctx.page);
                let mut cell_obj = json!({
                    "id": cell_id,
                    "row": c.row,
                    "col": c.col,
                    "text": c.text,
                    "bbox": {
                        "x": round4(c.bbox.x),
                        "y": round4(c.bbox.y),
                        "w": round4(c.bbox.width),
                        "h": round4(c.bbox.height),
                    },
                });
                if c.rowspan > 1 {
                    cell_obj["rowspan"] = json!(c.rowspan);
                }
                if c.colspan > 1 {
                    cell_obj["colspan"] = json!(c.colspan);
                }
                if c.is_header {
                    cell_obj["is_header"] = json!(true);
                }
                cell_obj
            })
            .collect();
        table_obj["cells"] = Value::Array(cells_json);
    }

    table_obj
}

struct CellInfo {
    row: u32,
    col: u32,
    rowspan: u32,
    colspan: u32,
    text: String,
    bbox: BoundingBox,
    is_header: bool,
}
