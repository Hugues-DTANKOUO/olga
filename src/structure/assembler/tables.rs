use std::collections::BTreeMap;

use super::types::CellAccumulator;
use super::*;

impl Assembler {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn route_to_table(
        &mut self,
        prim: &Primitive,
        page: u32,
        row: u32,
        col: u32,
        rowspan: u32,
        colspan: u32,
        is_header: bool,
        confidence: f32,
    ) {
        let text = Self::text_of(prim);

        let table = self.open_table.get_or_insert_with(|| OpenTable {
            cells: BTreeMap::new(),
            start_page: page,
            end_page: page,
            bbox: None,
            min_confidence: confidence,
            repeated_header_pages: Vec::new(),
            continuation_row_base: None,
        });

        if !table.repeated_header_pages.is_empty()
            && table.repeated_header_pages.contains(&page)
            && let Some(header_row) = table.cells.get(&0)
            && let Some(header_cell) = header_row.get(&col)
            && header_cell.text.trim() == text.trim()
        {
            return;
        }

        table.end_page = page;
        table.min_confidence = table.min_confidence.min(confidence);
        table.bbox = Some(match table.bbox {
            Some(b) => b.merge(&prim.bbox),
            None => prim.bbox,
        });

        let structure_source = Self::structure_source_for(prim);
        let row_map = table.cells.entry(row).or_default();

        if let Some(existing) = row_map.get(&col)
            && (existing.rowspan != rowspan || existing.colspan != colspan)
        {
            self.warnings.push(Warning {
                kind: WarningKind::PartialExtraction,
                message: format!(
                    "Table cell ({row},{col}): span mismatch — \
                     existing ({},{}) vs new ({rowspan},{colspan}). \
                     Keeping original span values.",
                    existing.rowspan, existing.colspan,
                ),
                page: Some(page),
            });
        }

        let cell = row_map.entry(col).or_insert_with(|| CellAccumulator {
            text: String::new(),
            rowspan,
            colspan,
            is_header,
            bbox: prim.bbox,
            page,
            confidence,
            structure_source,
        });

        if is_header {
            cell.is_header = true;
        }

        if !cell.text.is_empty() && !text.is_empty() {
            cell.text.push(' ');
        }
        cell.text.push_str(&text);
        cell.bbox = cell.bbox.merge(&prim.bbox);
        cell.confidence = cell.confidence.min(confidence);
    }

    pub(super) fn close_table(&mut self) {
        let mut table = match self.open_table.take() {
            Some(t) => t,
            None => return,
        };

        let bbox = table.bbox.unwrap_or(BoundingBox::new(0.0, 0.0, 1.0, 1.0));
        let pages = table.start_page..table.end_page + 1;

        let n_rows = table.cells.len() as u32;
        let n_cols = table
            .cells
            .values()
            .map(|row| row.keys().last().copied().map(|c| c + 1).unwrap_or(0))
            .max()
            .unwrap_or(0);

        let grid_cols: Option<u32> = if n_rows < 2 {
            None
        } else {
            let max_physical_cols = table
                .cells
                .values()
                .map(|row| row.len() as u32)
                .max()
                .unwrap_or(0);
            if max_physical_cols < 2 {
                None
            } else {
                Some(max_physical_cols.max(n_cols))
            }
        };

        if let Some(grid_cols) = grid_cols {
            for (&row_idx, cols) in &mut table.cells {
                for (&col_idx, cell) in cols.iter_mut() {
                    let max_colspan = grid_cols.saturating_sub(col_idx);
                    let max_rowspan = n_rows.saturating_sub(row_idx);
                    if cell.colspan > max_colspan {
                        self.warnings.push(Warning {
                            kind: WarningKind::HeuristicInference,
                            message: format!(
                                "Clamped colspan from {} to {} at cell ({}, {})",
                                cell.colspan, max_colspan, row_idx, col_idx
                            ),
                            page: Some(cell.page),
                        });
                        cell.colspan = max_colspan;
                    }
                    if cell.rowspan > max_rowspan {
                        self.warnings.push(Warning {
                            kind: WarningKind::HeuristicInference,
                            message: format!(
                                "Clamped rowspan from {} to {} at cell ({}, {})",
                                cell.rowspan, max_rowspan, row_idx, col_idx
                            ),
                            page: Some(cell.page),
                        });
                        cell.rowspan = max_rowspan;
                    }
                }
            }
        }

        let table_source = table
            .cells
            .values()
            .flat_map(|row| row.values())
            .next()
            .map(|c| c.structure_source.clone())
            .unwrap_or(StructureSource::HintAssembled);

        let mut table_node = DocumentNode::new(
            NodeKind::Table {
                rows: n_rows,
                cols: n_cols,
            },
            bbox,
            table.min_confidence,
            table_source,
            pages,
        );

        if !table.repeated_header_pages.is_empty() {
            let pages_str = table
                .repeated_header_pages
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(",");
            table_node
                .metadata
                .insert("repeated_header_pages".to_string(), pages_str);
        }

        for (&row_idx, cols) in &table.cells {
            let row_bbox = cols
                .values()
                .fold(None, |acc: Option<BoundingBox>, cell| {
                    Some(match acc {
                        Some(b) => b.merge(&cell.bbox),
                        None => cell.bbox,
                    })
                })
                .unwrap_or(bbox);

            let row_page = cols.values().map(|c| c.page).min().unwrap_or(0);
            let row_page_end = cols.values().map(|c| c.page).max().unwrap_or(0);
            let row_confidence = cols.values().map(|c| c.confidence).fold(1.0_f32, f32::min);
            let row_source = cols
                .values()
                .next()
                .map(|c| c.structure_source.clone())
                .unwrap_or(StructureSource::HintAssembled);

            let mut row_node = DocumentNode::new(
                NodeKind::TableRow,
                row_bbox,
                row_confidence,
                row_source,
                row_page..row_page_end + 1,
            );

            for (&col_idx, cell) in cols {
                let mut cell_node = DocumentNode::new(
                    NodeKind::TableCell {
                        row: row_idx,
                        col: col_idx,
                        rowspan: cell.rowspan,
                        colspan: cell.colspan,
                        text: cell.text.clone(),
                    },
                    cell.bbox,
                    cell.confidence,
                    cell.structure_source.clone(),
                    cell.page..cell.page + 1,
                );
                if cell.is_header {
                    cell_node
                        .metadata
                        .insert("role".to_string(), "header".to_string());
                }
                row_node.add_child(cell_node);
            }

            table_node.add_child(row_node);
        }

        self.emit_node(table_node);
    }
}
