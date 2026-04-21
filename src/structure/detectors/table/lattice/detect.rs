use super::*;

impl TableDetector {
    /// Detect tables using the lattice pipeline (Steps 1–5).
    ///
    /// This is the bordered-table path: edges → merge → intersections
    /// → cells → tables.
    pub(in crate::structure::detectors::table) fn detect_lattice(
        &self,
        page: &[Primitive],
    ) -> Vec<DetectedHint> {
        let raw_edges = edges::extract_edges(page, &self.edge_config);
        if raw_edges.is_empty() {
            return Vec::new();
        }

        let merged_edges = edges::snap_and_merge(raw_edges, &self.edge_config);

        let has_h = merged_edges
            .iter()
            .any(|e| e.orientation == edges::Orientation::Horizontal);
        let mut has_v = merged_edges
            .iter()
            .any(|e| e.orientation == edges::Orientation::Vertical);

        let mut all_edges = merged_edges;
        let mut has_virtual_v = false;
        if has_h && !has_v {
            all_edges.retain(|e| {
                if e.orientation == edges::Orientation::Horizontal {
                    let span = (e.end - e.start).abs();
                    span < 0.6
                } else {
                    true
                }
            });

            let still_has_h = all_edges
                .iter()
                .any(|e| e.orientation == edges::Orientation::Horizontal);
            if !still_has_h {
                return Vec::new();
            }

            let virtual_v = self.generate_virtual_vertical_edges(&all_edges, page);
            if virtual_v.is_empty() {
                return Vec::new();
            }
            all_edges.extend(virtual_v);
            has_virtual_v = true;
            has_v = all_edges
                .iter()
                .any(|e| e.orientation == edges::Orientation::Vertical);
        }

        if !has_h || !has_v {
            return Vec::new();
        }

        let validation = if has_virtual_v {
            edges::CellValidation::Relaxed
        } else {
            self.edge_config.cell_validation
        };

        self.edges_to_hints(&all_edges, page, self.lattice_confidence, validation)
    }

    /// Convert a set of edges into table detection hints using the standard
    /// 5-step pipeline (Steps 3–5: intersections → cells → tables).
    ///
    /// This is the shared core used by both the lattice path (explicit edges)
    /// and the stream path (virtual edges from text alignment). Parameters:
    /// - `confidence`: score attached to resulting hints (0.85 lattice, 0.65 stream)
    /// - `cell_validation`: Strict for pure lattice (all edges explicit),
    ///   Relaxed for stream and partial grid (virtual edges are approximate
    ///   and may not form perfect 4-corner cells)
    pub(in crate::structure::detectors::table) fn edges_to_hints(
        &self,
        edges: &[edges::Edge],
        page: &[Primitive],
        confidence: f32,
        cell_validation: edges::CellValidation,
    ) -> Vec<DetectedHint> {
        let ints = intersections::find_intersections(edges, self.intersection_tolerance);
        if ints.len() < 4 {
            return Vec::new();
        }

        let raw_cells =
            cells::construct_cells(&ints, edges, self.intersection_tolerance, cell_validation);
        if raw_cells.is_empty() {
            return Vec::new();
        }

        let tables = cells::group_into_tables(
            &raw_cells,
            page,
            self.intersection_tolerance,
            self.min_rows,
            self.min_columns,
        );

        let mut hints = Vec::new();
        for table in &tables {
            let assignments = cells::assign_text_to_table(table, page);
            let is_header_row = self.detect_header_row(&assignments, page, table);
            let with_spans = self.infer_lattice_spans(&assignments, page, table);

            for (idx, row, col, rowspan, colspan) in with_spans {
                if page[idx].has_format_hints() {
                    continue;
                }

                let kind = if row == 0 && is_header_row {
                    HintKind::TableHeader { col: col as u32 }
                } else {
                    HintKind::TableCell {
                        row: row as u32,
                        col: col as u32,
                        rowspan,
                        colspan,
                    }
                };

                hints.push(DetectedHint {
                    primitive_index: idx,
                    hint: SemanticHint::from_heuristic(kind, confidence, "TableDetector"),
                });
            }
        }

        hints
    }
}
