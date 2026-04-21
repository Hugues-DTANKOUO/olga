use super::super::stream::{TextItem, cluster_x_positions};
use super::*;

impl TableDetector {
    /// Generate virtual vertical edges from text X-clustering.
    ///
    /// When a page has horizontal rules but no vertical rules (partial grid),
    /// this method infers column boundaries from text X-position clustering
    /// and creates synthetic vertical edges that span the full height of the
    /// horizontal rule region. These edges are fed into the standard 5-step
    /// pipeline, unifying the partial grid path with the lattice path.
    pub(super) fn generate_virtual_vertical_edges(
        &self,
        h_edges: &[edges::Edge],
        page: &[Primitive],
    ) -> Vec<edges::Edge> {
        let h_lines: Vec<&edges::Edge> = h_edges
            .iter()
            .filter(|e| e.orientation == edges::Orientation::Horizontal)
            .collect();

        if h_lines.len() <= self.min_rows {
            return Vec::new();
        }

        let mut h_ys: Vec<f32> = h_lines.iter().map(|e| e.position).collect();
        h_ys.sort_by(|a, b| a.total_cmp(b));
        h_ys.dedup_by(|a, b| (*a - *b).abs() < self.intersection_tolerance);

        let y_min = h_ys[0];
        let y_max = h_ys[h_ys.len() - 1];

        let text_items: Vec<TextItem> = page
            .iter()
            .filter_map(|p| {
                if p.has_format_hints() {
                    return None;
                }
                if let PrimitiveKind::Text { ref content, .. } = p.kind {
                    if content.trim().is_empty() {
                        return None;
                    }
                    let cy = p.bbox.y + p.bbox.height / 2.0;
                    if cy >= y_min && cy <= y_max {
                        Some(TextItem {
                            x: p.bbox.x,
                            y: p.bbox.y,
                            width: p.bbox.width,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        if text_items.len() < self.min_columns * self.min_rows {
            return Vec::new();
        }

        let x_clusters = cluster_x_positions(&text_items, self.x_tolerance);
        if x_clusters.len() < self.min_columns {
            return Vec::new();
        }

        let x_max_text = page
            .iter()
            .filter_map(|p| {
                if p.has_format_hints() {
                    return None;
                }
                if let PrimitiveKind::Text { ref content, .. } = p.kind
                    && !content.trim().is_empty()
                {
                    let cy = p.bbox.y + p.bbox.height / 2.0;
                    if cy >= y_min && cy <= y_max {
                        return Some(p.bbox.right());
                    }
                }
                None
            })
            .fold(0.0_f32, f32::max);

        let mut v_edges = Vec::with_capacity(x_clusters.len() + 1);

        for &x in &x_clusters {
            let x_pos = x - self.x_tolerance * 0.5;
            v_edges.push(edges::Edge {
                orientation: edges::Orientation::Vertical,
                position: x_pos,
                start: y_min,
                end: y_max,
            });
        }

        let right_edge_x = x_max_text + self.x_tolerance * 0.5;
        v_edges.push(edges::Edge {
            orientation: edges::Orientation::Vertical,
            position: right_edge_x,
            start: y_min,
            end: y_max,
        });

        v_edges
    }
}
