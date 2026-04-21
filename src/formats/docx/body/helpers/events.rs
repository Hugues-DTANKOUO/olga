use std::collections::{BTreeMap, HashSet};

use quick_xml::events::BytesStart;

use crate::error::{Warning, WarningKind};
use crate::formats::xml_utils::{attr_value, is_mc_element, is_w_element, local_name};

use super::super::paragraphs::{capture_docx_drawing_alt_text, capture_docx_drawing_link};
use super::super::state::{
    CellFinalizeWarningScope, DocxWarningContext, DrawingContext, FlattenedTableContext,
    ScopedIssueKind, ScopedIssueSummary, WarningScopeRef,
};
use super::super::tables::{ActiveTableState, FlatteningSummary};
use super::super::warnings::{
    docx_story_scope_location, docx_warning_scope, docx_warning_scope_location,
    media_ambiguity_story_scope, record_scoped_issue_summary,
};
use super::super::{PartContext, Relationship};
use super::context::story_id_from_element;

pub(crate) fn handle_docx_mc_choice_start(
    name: &[u8],
    qname: &[u8],
    in_mc_choice: &mut bool,
    skip_depth: &mut u32,
) -> bool {
    if name == b"Choice" && is_mc_element(qname) {
        *in_mc_choice = true;
        *skip_depth = 1;
        return true;
    }
    if *in_mc_choice {
        *skip_depth += 1;
        return true;
    }
    false
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_docx_table_start(
    table_stack: &mut Vec<ActiveTableState>,
    flattening_summaries: &mut BTreeMap<(u64, u32, u32), FlatteningSummary>,
    warnings: &mut Vec<Warning>,
    warning_dedup_keys: &mut HashSet<String>,
    part_ctx: PartContext,
    current_story_id: Option<&str>,
    current_page: u32,
    next_table_id: &mut u64,
) {
    let nested_table_context: Option<Vec<FlattenedTableContext>> = table_stack
        .last()
        .and_then(|table| table.current_flattened_child_context());
    if let Some(context) = nested_table_context.as_ref().and_then(|c| c.last()) {
        let story_location = docx_story_scope_location(part_ctx, current_story_id, current_page);
        super::super::warnings::push_unique_warning(
            warnings,
            warning_dedup_keys,
            format!(
                "nested_table_flattened:{}:{}:{}",
                story_location, context.row, context.col
            ),
            Warning {
                kind: WarningKind::PartialExtraction,
                message: format!(
                    "DOCX {} table cell ({}, {}) contains a nested table; inner table semantics are preserved, but parent cell containment is flattened in the Primitive layer",
                    story_location, context.row, context.col
                ),
                page: Some(current_page),
            },
        );
    }
    if let Some(root_context) = nested_table_context
        .as_ref()
        .and_then(|contexts| contexts.first())
    {
        let key = (root_context.table_id, root_context.row, root_context.col);
        let summary = flattening_summaries
            .entry(key)
            .or_insert(FlatteningSummary {
                location: format!(
                    "{} table cell ({}, {})",
                    docx_story_scope_location(part_ctx, current_story_id, current_page),
                    root_context.row,
                    root_context.col
                ),
                page: current_page,
                nested_table_count: 0,
                max_depth: 0,
            });
        summary.nested_table_count += 1;
        summary.max_depth = summary.max_depth.max(
            nested_table_context
                .as_ref()
                .and_then(|contexts| contexts.last())
                .map_or(0, |context| context.depth),
        );
    }

    let table_id = *next_table_id;
    *next_table_id += 1;
    table_stack.push(ActiveTableState::new(
        table_id,
        nested_table_context.unwrap_or_default(),
    ));
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_docx_hyperlink_start(
    e: &BytesStart<'_>,
    rels: &std::collections::HashMap<String, Relationship>,
    table_stack: &[ActiveTableState],
    part_ctx: PartContext,
    current_story_id: Option<&str>,
    current_page: u32,
    hyperlink_url: &mut Option<String>,
    warnings: &mut Vec<Warning>,
    scoped_issue_summaries: &mut BTreeMap<String, ScopedIssueSummary>,
) {
    for attr in e.attributes().flatten() {
        let k = local_name(attr.key.as_ref());
        if k == b"id" {
            let rid = attr_value(&attr);
            if let Some(rel) = rels.get(&rid) {
                if rel.is_external {
                    *hyperlink_url = Some(rel.target.clone());
                } else {
                    let (scope_key, warning_scope) =
                        docx_warning_scope(table_stack, part_ctx, current_story_id, current_page);
                    warnings.push(Warning {
                        kind: WarningKind::PartialExtraction,
                        message: format!(
                            "DOCX {} references internal hyperlink relationship '{}' which is not surfaced as a link hint",
                            warning_scope, rid
                        ),
                        page: Some(current_page),
                    });
                    record_scoped_issue_summary(
                        scoped_issue_summaries,
                        &scope_key,
                        &warning_scope,
                        current_page,
                        ScopedIssueKind::InternalRelationshipNotSurfaced,
                    );
                }
            } else {
                let (scope_key, warning_scope) =
                    docx_warning_scope(table_stack, part_ctx, current_story_id, current_page);
                warnings.push(Warning {
                    kind: WarningKind::UnresolvedRelationship,
                    message: format!(
                        "DOCX {} references hyperlink relationship '{}' which was not found in document relationships",
                        warning_scope, rid
                    ),
                    page: Some(current_page),
                });
                record_scoped_issue_summary(
                    scoped_issue_summaries,
                    &scope_key,
                    &warning_scope,
                    current_page,
                    ScopedIssueKind::UnresolvedRelationship,
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_docx_tracked_change_start(
    name: &[u8],
    qname: &[u8],
    table_stack: &[ActiveTableState],
    part_ctx: PartContext,
    current_story_id: Option<&str>,
    current_page: u32,
    in_del: &mut bool,
    warnings: &mut Vec<Warning>,
    scoped_issue_summaries: &mut BTreeMap<String, ScopedIssueSummary>,
) {
    if !is_w_element(qname) {
        return;
    }

    let Some((message, issue_kind, set_in_del)) = (match name {
        b"del" => Some((
            "skipped a deletion (tracked change)",
            ScopedIssueKind::TrackedDeletionSkipped,
            true,
        )),
        b"ins" => Some((
            "accepted an insertion (tracked change)",
            ScopedIssueKind::TrackedInsertionAccepted,
            false,
        )),
        _ => None,
    }) else {
        return;
    };

    if set_in_del {
        *in_del = true;
    }
    let (scope_key, warning_scope) =
        docx_warning_scope(table_stack, part_ctx, current_story_id, current_page);
    warnings.push(Warning {
        kind: WarningKind::TrackedChangeResolved,
        message: format!("DOCX {} {}", warning_scope, message),
        page: Some(current_page),
    });
    record_scoped_issue_summary(
        scoped_issue_summaries,
        &scope_key,
        &warning_scope,
        current_page,
        issue_kind,
    );
}

pub(crate) fn handle_docx_drawing_alt_text(
    current_drawing: Option<&mut DrawingContext>,
    e: &BytesStart<'_>,
) {
    if let Some(drawing) = current_drawing {
        capture_docx_drawing_alt_text(drawing, e);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_docx_drawing_hyperlink(
    current_drawing: Option<&mut DrawingContext>,
    e: &BytesStart<'_>,
    rels: &std::collections::HashMap<String, Relationship>,
    table_stack: &[ActiveTableState],
    part_ctx: PartContext,
    current_story_id: Option<&str>,
    current_page: u32,
    warnings: &mut Vec<Warning>,
    scoped_issue_summaries: &mut BTreeMap<String, ScopedIssueSummary>,
    warning_dedup_keys: &mut HashSet<String>,
) {
    let Some(drawing) = current_drawing else {
        return;
    };

    let (scope_key, warning_scope) =
        docx_warning_scope(table_stack, part_ctx, current_story_id, current_page);
    capture_docx_drawing_link(
        drawing,
        e,
        rels,
        warnings,
        current_page,
        &scope_key,
        &warning_scope,
        scoped_issue_summaries,
        warning_dedup_keys,
    );
}

pub(crate) fn handle_page_break(
    name: &[u8],
    e: &BytesStart<'_>,
    current_page: &mut u32,
    y_position: &mut f32,
) {
    if name != b"br" || !is_w_element(e.name().as_ref()) {
        return;
    }

    for attr in e.attributes().flatten() {
        if local_name(attr.key.as_ref()) == b"type" && attr_value(&attr) == "page" {
            *current_page += 1;
            *y_position = 0.0;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_docx_reference(
    para: &mut super::super::ParagraphContext,
    name: &[u8],
    e: &BytesStart<'_>,
    table_stack: &[ActiveTableState],
    part_ctx: PartContext,
    current_story_id: Option<&str>,
    current_page: u32,
    warnings: &mut Vec<Warning>,
    scoped_issue_summaries: &mut BTreeMap<String, ScopedIssueSummary>,
) {
    match name {
        b"footnoteReference" | b"endnoteReference" if is_w_element(e.name().as_ref()) => {
            if let Some(id) = story_id_from_element(e) {
                para.footnote_refs.push(id);
            } else {
                let (scope_key, warning_scope) =
                    docx_warning_scope(table_stack, part_ctx, current_story_id, current_page);
                warnings.push(Warning {
                    kind: WarningKind::MalformedContent,
                    message: format!(
                        "DOCX {} contains a footnote/endnote reference missing w:id",
                        warning_scope
                    ),
                    page: Some(current_page),
                });
                record_scoped_issue_summary(
                    scoped_issue_summaries,
                    &scope_key,
                    &warning_scope,
                    current_page,
                    ScopedIssueKind::MissingReferenceId,
                );
            }
        }
        b"commentReference" if is_w_element(e.name().as_ref()) => {
            if let Some(id) = story_id_from_element(e) {
                para.comment_refs.push(id);
            } else {
                let (scope_key, warning_scope) =
                    docx_warning_scope(table_stack, part_ctx, current_story_id, current_page);
                warnings.push(Warning {
                    kind: WarningKind::MalformedContent,
                    message: format!(
                        "DOCX {} contains a comment reference missing w:id",
                        warning_scope
                    ),
                    page: Some(current_page),
                });
                record_scoped_issue_summary(
                    scoped_issue_summaries,
                    &scope_key,
                    &warning_scope,
                    current_page,
                    ScopedIssueKind::MissingReferenceId,
                );
            }
        }
        _ => {}
    }
}

pub(crate) fn finalize_active_table_cell_properties(
    table_stack: &mut [ActiveTableState],
    part_ctx: PartContext,
    current_story_id: Option<&str>,
    current_page: u32,
    warnings: &mut Vec<Warning>,
    scoped_issue_summaries: &mut BTreeMap<String, ScopedIssueSummary>,
    warning_dedup_keys: &mut HashSet<String>,
) {
    let warning_scope =
        docx_warning_scope_location(table_stack, part_ctx, current_story_id, current_page);
    let (summary_scope_key, summary_scope_location) =
        media_ambiguity_story_scope(part_ctx, current_story_id, current_page);
    if let Some(table) = table_stack.last_mut() {
        let mut warning_ctx = DocxWarningContext {
            warnings,
            current_page,
            scoped_issue_summaries,
            warning_dedup_keys,
        };
        table.finalize_current_cell_properties(
            &mut warning_ctx,
            CellFinalizeWarningScope {
                warning: WarningScopeRef {
                    key: &summary_scope_key,
                    location: &warning_scope,
                },
                summary: WarningScopeRef {
                    key: &summary_scope_key,
                    location: &summary_scope_location,
                },
            },
        );
    }
}
