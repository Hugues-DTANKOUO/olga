use std::collections::HashMap;

use crate::model::Primitive;

use super::spatial::unique_positions;
use super::{Cell, Table};

/// Group cells into tables using connected-component analysis (Step 5).
///
/// Two cells are connected if they share a corner (within tolerance). Uses
/// a corner-indexed HashMap for O(n) grouping instead of O(n²) pairwise
/// comparison.
///
/// Post-processing filters:
/// - Tables with fewer than 2 rows or 2 columns are discarded.
/// - Tables that contain no text primitives are discarded (graphic false positives).
pub fn group_into_tables(
    cells: &[Cell],
    page: &[Primitive],
    tolerance: f32,
    min_rows: usize,
    min_cols: usize,
) -> Vec<Table> {
    if cells.is_empty() {
        return Vec::new();
    }

    let n = cells.len();
    let mut parent: Vec<usize> = (0..n).collect();
    let mut rank: Vec<usize> = vec![0; n];

    fn uf_find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }

    fn uf_union(parent: &mut [usize], rank: &mut [usize], a: usize, b: usize) {
        let ra = uf_find(parent, a);
        let rb = uf_find(parent, b);
        if ra == rb {
            return;
        }
        if rank[ra] < rank[rb] {
            parent[ra] = rb;
        } else if rank[ra] > rank[rb] {
            parent[rb] = ra;
        } else {
            parent[rb] = ra;
            rank[ra] += 1;
        }
    }

    let quant = |v: f32| -> i64 { (v / tolerance).floor() as i64 };
    let mut corner_to_cells: HashMap<(i64, i64), Vec<usize>> = HashMap::new();

    for (i, cell) in cells.iter().enumerate() {
        let corners = [
            (cell.x0, cell.y0),
            (cell.x1, cell.y0),
            (cell.x0, cell.y1),
            (cell.x1, cell.y1),
        ];
        for &(cx, cy) in &corners {
            let qx = quant(cx);
            let qy = quant(cy);
            for dx in 0..=1_i64 {
                for dy in 0..=1_i64 {
                    corner_to_cells
                        .entry((qx + dx, qy + dy))
                        .or_default()
                        .push(i);
                }
            }
        }
    }

    for bucket in corner_to_cells.values() {
        if bucket.len() > 1 {
            let first = bucket[0];
            for &other in &bucket[1..] {
                uf_union(&mut parent, &mut rank, first, other);
            }
        }
    }

    let mut components: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..n {
        let root = uf_find(&mut parent, i);
        components.entry(root).or_default().push(i);
    }

    let mut tables = Vec::new();

    for indices in components.values() {
        let component_cells: Vec<Cell> = indices.iter().map(|&i| cells[i].clone()).collect();

        let mut all_xs: Vec<f32> = Vec::new();
        let mut all_ys: Vec<f32> = Vec::new();
        for cell in &component_cells {
            all_xs.push(cell.x0);
            all_xs.push(cell.x1);
            all_ys.push(cell.y0);
            all_ys.push(cell.y1);
        }

        let col_xs = unique_positions(all_xs.into_iter(), tolerance);
        let row_ys = unique_positions(all_ys.into_iter(), tolerance);

        let table = Table {
            cells: component_cells,
            row_ys,
            col_xs,
        };

        if table.num_rows() < min_rows || table.num_cols() < min_cols {
            continue;
        }

        let (bx0, by0, bx1, by1) = table.bbox();
        let has_text = page.iter().any(|p| {
            if let crate::model::PrimitiveKind::Text { ref content, .. } = p.kind {
                if content.trim().is_empty() {
                    return false;
                }
                let cx = p.bbox.x + p.bbox.width / 2.0;
                let cy = p.bbox.y + p.bbox.height / 2.0;
                cx >= bx0 && cx <= bx1 && cy >= by0 && cy <= by1
            } else {
                false
            }
        });

        if !has_text {
            continue;
        }

        tables.push(table);
    }

    tables.sort_by(|a, b| {
        let (ax0, ay0, _, _) = a.bbox();
        let (bx0, by0, _, _) = b.bbox();
        ay0.total_cmp(&by0).then(ax0.total_cmp(&bx0))
    });

    tables
}
