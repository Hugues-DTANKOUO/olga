use std::collections::HashMap;

use crate::error::{Warning, WarningKind};
use crate::model::{HintKind, Primitive, SemanticHint};
use crate::pipeline::PrimitiveSink;

use super::super::types::TableContext;
use super::helpers::push_hint_once;
use super::state::{
    CellFinalizeWarningScope, DocxWarningContext, FlattenedTableContext, ScopedIssueKind,
};
use super::warnings::{push_unique_warning, record_scoped_issue_summary};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum VerticalMergeRole {
    None,
    Restart,
    Continue,
}

#[derive(Debug, Clone)]
pub(super) struct FlatteningSummary {
    pub(super) location: String,
    pub(super) page: u32,
    pub(super) nested_table_count: u32,
    pub(super) max_depth: u32,
}

#[derive(Debug, Clone)]
struct BufferedTableCell {
    row: u32,
    col: u32,
    rowspan: u32,
    colspan: u32,
    is_header: bool,
    primitive_indices: Vec<usize>,
}

#[derive(Debug, Clone, Copy)]
struct VerticalMergeOrigin {
    record_idx: usize,
    has_explicit_restart: bool,
}

#[derive(Debug, Clone)]
pub(super) struct ActiveTableState {
    pub(super) table_id: u64,
    layout: TableContext,
    current_cell_vmerge: VerticalMergeRole,
    current_cell_record_idx: Option<usize>,
    in_cell: bool,
    pending_primitives: Vec<Primitive>,
    buffered_cells: Vec<BufferedTableCell>,
    active_vertical_merges: HashMap<u32, VerticalMergeOrigin>,
    flattened_ancestry: Vec<FlattenedTableContext>,
}

impl ActiveTableState {
    pub(super) fn new(table_id: u64, flattened_ancestry: Vec<FlattenedTableContext>) -> Self {
        Self {
            table_id,
            layout: TableContext::new(),
            current_cell_vmerge: VerticalMergeRole::None,
            current_cell_record_idx: None,
            in_cell: false,
            pending_primitives: Vec::new(),
            buffered_cells: Vec::new(),
            active_vertical_merges: HashMap::new(),
            flattened_ancestry,
        }
    }

    pub(super) fn start_row(&mut self) {
        self.layout.current_col = 0;
        self.layout.current_row_is_header = false;
    }

    pub(super) fn start_cell(&mut self) {
        self.in_cell = true;
        self.layout.current_cell_colspan = 1;
        self.current_cell_vmerge = VerticalMergeRole::None;
        self.current_cell_record_idx = None;
    }

    pub(super) fn note_header_row(&mut self, enabled: bool) {
        self.layout.current_row_is_header = enabled;
    }

    pub(super) fn set_current_colspan(&mut self, colspan: u32) {
        self.layout.current_cell_colspan = colspan.max(1);
    }

    pub(super) fn set_vertical_merge_role(&mut self, role: VerticalMergeRole) {
        self.current_cell_vmerge = role;
    }

    pub(super) fn finalize_current_cell_properties(
        &mut self,
        warning_ctx: &mut DocxWarningContext<'_>,
        scope: CellFinalizeWarningScope<'_>,
    ) {
        if self.current_cell_record_idx.is_some() {
            return;
        }

        match self.current_cell_vmerge {
            VerticalMergeRole::None => {}
            VerticalMergeRole::Restart => {
                let idx = self.ensure_current_cell_record();
                for col in self.layout.current_col
                    ..self.layout.current_col + self.layout.current_cell_colspan.max(1)
                {
                    self.active_vertical_merges.insert(
                        col,
                        VerticalMergeOrigin {
                            record_idx: idx,
                            has_explicit_restart: true,
                        },
                    );
                }
            }
            VerticalMergeRole::Continue => {
                let Some(origin) = self
                    .active_vertical_merges
                    .get(&self.layout.current_col)
                    .copied()
                else {
                    push_unique_warning(
                        warning_ctx.warnings,
                        warning_ctx.warning_dedup_keys,
                        format!("vmerge_without_restart:{}", scope.summary.key),
                        Warning {
                            kind: WarningKind::PartialExtraction,
                            message: format!(
                                "DOCX {} continues a vertical merge without a matching restart cell",
                                scope.warning.location
                            ),
                            page: Some(warning_ctx.current_page),
                        },
                    );
                    record_scoped_issue_summary(
                        warning_ctx.scoped_issue_summaries,
                        scope.summary.key,
                        scope.summary.location,
                        warning_ctx.current_page,
                        ScopedIssueKind::VerticalMergeWithoutRestart,
                    );
                    let idx = self.ensure_current_cell_record();
                    for col in self.layout.current_col
                        ..self.layout.current_col + self.layout.current_cell_colspan.max(1)
                    {
                        self.active_vertical_merges.insert(
                            col,
                            VerticalMergeOrigin {
                                record_idx: idx,
                                has_explicit_restart: false,
                            },
                        );
                    }
                    return;
                };

                if !origin.has_explicit_restart {
                    push_unique_warning(
                        warning_ctx.warnings,
                        warning_ctx.warning_dedup_keys,
                        format!("vmerge_without_restart:{}", scope.summary.key),
                        Warning {
                            kind: WarningKind::PartialExtraction,
                            message: format!(
                                "DOCX {} continues a vertical merge without a matching restart cell",
                                scope.warning.location
                            ),
                            page: Some(warning_ctx.current_page),
                        },
                    );
                    record_scoped_issue_summary(
                        warning_ctx.scoped_issue_summaries,
                        scope.summary.key,
                        scope.summary.location,
                        warning_ctx.current_page,
                        ScopedIssueKind::VerticalMergeWithoutRestart,
                    );
                }

                let origin_idx = origin.record_idx;
                let origin = &mut self.buffered_cells[origin_idx];
                if origin.colspan != self.layout.current_cell_colspan.max(1) {
                    push_unique_warning(
                        warning_ctx.warnings,
                        warning_ctx.warning_dedup_keys,
                        format!(
                            "vmerge_colspan_mismatch:{}:{}:{}",
                            scope.summary.key,
                            self.layout.current_cell_colspan.max(1),
                            origin.colspan
                        ),
                        Warning {
                            kind: WarningKind::PartialExtraction,
                            message: format!(
                                "DOCX {} continues a vertical merge with colspan {} but the restart cell used colspan {}",
                                scope.warning.location,
                                self.layout.current_cell_colspan.max(1),
                                origin.colspan
                            ),
                            page: Some(warning_ctx.current_page),
                        },
                    );
                    record_scoped_issue_summary(
                        warning_ctx.scoped_issue_summaries,
                        scope.summary.key,
                        scope.summary.location,
                        warning_ctx.current_page,
                        ScopedIssueKind::VerticalMergeColspanMismatch,
                    );
                }
                origin.rowspan += 1;
                self.current_cell_record_idx = Some(origin_idx);
            }
        }
    }

    fn ensure_current_cell_record(&mut self) -> usize {
        if let Some(idx) = self.current_cell_record_idx {
            return idx;
        }

        let idx = self.buffered_cells.len();
        self.buffered_cells.push(BufferedTableCell {
            row: self.layout.current_row,
            col: self.layout.current_col,
            rowspan: 1,
            colspan: self.layout.current_cell_colspan.max(1),
            is_header: self.layout.current_row_is_header,
            primitive_indices: Vec::new(),
        });
        self.current_cell_record_idx = Some(idx);
        idx
    }

    pub(super) fn push_pending_primitive(&mut self, primitive: Primitive) {
        let primitive_idx = self.pending_primitives.len();
        let cell_idx = self.ensure_current_cell_record();
        self.pending_primitives.push(primitive);
        self.buffered_cells[cell_idx]
            .primitive_indices
            .push(primitive_idx);
    }

    pub(super) fn finish_cell(&mut self) {
        self.in_cell = false;
        self.layout.current_col += self.layout.current_cell_colspan.max(1);
        self.layout.current_cell_colspan = 1;
        self.current_cell_vmerge = VerticalMergeRole::None;
        self.current_cell_record_idx = None;
    }

    pub(super) fn finish_row(&mut self) {
        self.layout.current_row += 1;
    }

    fn annotate_flattened_table_primitives(&mut self) {
        if self.flattened_ancestry.is_empty() {
            return;
        }

        for primitive in &mut self.pending_primitives {
            for flattened_parent_cell in &self.flattened_ancestry {
                push_hint_once(
                    primitive,
                    SemanticHint::from_format(HintKind::ContainedInTableCell {
                        row: flattened_parent_cell.row,
                        col: flattened_parent_cell.col,
                        depth: flattened_parent_cell.depth,
                    }),
                );
                push_hint_once(
                    primitive,
                    SemanticHint::from_format(HintKind::FlattenedFromTableCell {
                        row: flattened_parent_cell.row,
                        col: flattened_parent_cell.col,
                        depth: flattened_parent_cell.depth,
                    }),
                );
            }
        }
    }

    pub(super) fn flush_pending_primitives(mut self, primitives: &mut impl PrimitiveSink) {
        self.annotate_flattened_table_primitives();
        for cell in &self.buffered_cells {
            for &primitive_idx in &cell.primitive_indices {
                if let Some(primitive) = self.pending_primitives.get_mut(primitive_idx) {
                    primitive.hints.retain(|hint| {
                        !matches!(
                            hint.kind,
                            HintKind::TableCell { .. } | HintKind::TableHeader { .. }
                        )
                    });
                    let table_hint = if cell.is_header {
                        SemanticHint::from_format(HintKind::TableHeader { col: cell.col })
                    } else {
                        SemanticHint::from_format(HintKind::TableCell {
                            row: cell.row,
                            col: cell.col,
                            rowspan: cell.rowspan,
                            colspan: cell.colspan,
                        })
                    };
                    push_hint_once(
                        primitive,
                        SemanticHint::from_format(HintKind::ContainedInTableCell {
                            row: cell.row,
                            col: cell.col,
                            depth: self.flattened_ancestry.len() as u32,
                        }),
                    );
                    push_hint_once(primitive, table_hint);
                }
            }
        }

        for primitive in self.pending_primitives {
            primitives.emit(primitive);
        }
    }

    pub(super) fn append_into_parent(mut self, parent: &mut ActiveTableState) {
        self.annotate_flattened_table_primitives();
        let offset = parent.pending_primitives.len();
        for cell in &mut self.buffered_cells {
            for primitive_idx in &mut cell.primitive_indices {
                *primitive_idx += offset;
            }
        }
        parent.pending_primitives.extend(self.pending_primitives);
        parent.buffered_cells.extend(self.buffered_cells);
    }

    pub(super) fn current_flattened_child_context(&self) -> Option<Vec<FlattenedTableContext>> {
        self.in_cell.then(|| {
            let mut ancestry = self.flattened_ancestry.clone();
            ancestry.push(FlattenedTableContext {
                table_id: self.table_id,
                row: self.layout.current_row,
                col: self.layout.current_col,
                depth: ancestry.len() as u32 + 1,
            });
            ancestry
        })
    }

    pub(super) fn current_cell_warning_scope(&self) -> Option<(String, String)> {
        self.in_cell.then(|| {
            (
                format!(
                    "table:{}:{}:{}",
                    self.table_id, self.layout.current_row, self.layout.current_col
                ),
                format!(
                    "table cell ({}, {})",
                    self.layout.current_row, self.layout.current_col
                ),
            )
        })
    }
}
