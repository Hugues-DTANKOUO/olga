use super::super::edges::{CellValidation, Edge, Orientation};
use super::super::intersections::Intersection;
use super::Cell;
use super::spatial::{build_edge_spans, build_intersection_index, spans_cover, unique_positions};

/// Construct cells from intersection points (Step 4).
///
/// For each intersection `(x_i, y_i)`, find the nearest intersection to the
/// right on the same Y, below on the same X, and diagonally. If all four
/// corners exist and are connected by edges, a cell is formed.
///
/// Uses spatial indexes for O(1) intersection lookups and efficient edge
/// connectivity checks instead of O(n) linear scans. The four-corner
/// cell-from-intersections approach is informed by pdfplumber's
/// `_cells_from_intersections`; see ATTRIBUTION.md.
pub fn construct_cells(
    intersections: &[Intersection],
    edges: &[Edge],
    tolerance: f32,
    validation: CellValidation,
) -> Vec<Cell> {
    if intersections.len() < 4 {
        return Vec::new();
    }

    let ys = unique_positions(intersections.iter().map(|i| i.y), tolerance);
    let xs = unique_positions(intersections.iter().map(|i| i.x), tolerance);

    if xs.len() < 2 || ys.len() < 2 {
        return Vec::new();
    }

    let int_set = build_intersection_index(intersections, &xs, &ys, tolerance);
    let h_spans = build_edge_spans(&ys, edges, Orientation::Horizontal, tolerance);
    let v_spans = build_edge_spans(&xs, edges, Orientation::Vertical, tolerance);

    let mut cells = Vec::new();

    for yi in 0..ys.len() - 1 {
        for xi in 0..xs.len() - 1 {
            if !int_set.contains(&(yi, xi)) {
                continue;
            }

            let mut found_cell = false;
            for xi2 in (xi + 1)..xs.len() {
                if !int_set.contains(&(yi, xi2)) {
                    continue;
                }
                for yi2 in (yi + 1)..ys.len() {
                    if !int_set.contains(&(yi2, xi)) || !int_set.contains(&(yi2, xi2)) {
                        continue;
                    }

                    let x0 = xs[xi];
                    let x1 = xs[xi2];
                    let y0 = ys[yi];
                    let y1 = ys[yi2];

                    let top = spans_cover(&h_spans[yi], x0, x1, tolerance);
                    let bottom = spans_cover(&h_spans[yi2], x0, x1, tolerance);
                    let left = spans_cover(&v_spans[xi], y0, y1, tolerance);
                    let right = spans_cover(&v_spans[xi2], y0, y1, tolerance);

                    let valid = match validation {
                        CellValidation::Strict => top && bottom && left && right,
                        CellValidation::Relaxed => (top && bottom) || (left && right),
                    };

                    if valid {
                        cells.push(Cell { x0, y0, x1, y1 });
                        found_cell = true;
                        break;
                    }
                }
                if found_cell {
                    break;
                }
            }
        }
    }

    cells
}
