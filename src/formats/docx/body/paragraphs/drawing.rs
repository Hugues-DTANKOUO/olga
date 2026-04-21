use std::collections::{BTreeMap, HashMap, HashSet};

use crate::error::{Warning, WarningKind};
use crate::formats::xml_utils::{attr_value, local_name};

use super::super::Relationship;
use super::super::state::{DrawingContext, ScopedIssueKind, ScopedIssueSummary};
use super::super::warnings::{push_unique_warning, record_scoped_issue_summary};

pub(in super::super) fn capture_docx_drawing_alt_text(
    drawing: &mut DrawingContext,
    e: &quick_xml::events::BytesStart<'_>,
) {
    let mut title = None;
    let mut descr = None;
    for attr in e.attributes().flatten() {
        let key = local_name(attr.key.as_ref());
        let value = attr_value(&attr).trim().to_string();
        if value.is_empty() {
            continue;
        }
        match key.as_slice() {
            b"descr" => descr = Some(value),
            b"title" => title = Some(value),
            _ => {}
        }
    }
    if drawing.alt_text.is_none() {
        drawing.alt_text = descr.or(title);
    }
}

#[allow(clippy::too_many_arguments)]
pub(in super::super) fn capture_docx_drawing_link(
    drawing: &mut DrawingContext,
    e: &quick_xml::events::BytesStart<'_>,
    rels: &HashMap<String, Relationship>,
    warnings: &mut Vec<Warning>,
    current_page: u32,
    warning_scope_key: &str,
    warning_scope: &str,
    scoped_issue_summaries: &mut BTreeMap<String, ScopedIssueSummary>,
    warning_dedup_keys: &mut HashSet<String>,
) {
    let mut rel_id = None;
    for attr in e.attributes().flatten() {
        if local_name(attr.key.as_ref()) == b"id" {
            rel_id = Some(attr_value(&attr));
            break;
        }
    }
    let Some(rel_id) = rel_id else {
        return;
    };

    match rels.get(&rel_id) {
        Some(rel) if rel.is_external => drawing.hyperlink_url = Some(rel.target.clone()),
        Some(_) => {
            push_unique_warning(
                warnings,
                warning_dedup_keys,
                format!("internal_drawing_link_not_surfaced:{warning_scope_key}:{rel_id}"),
                Warning {
                    kind: WarningKind::PartialExtraction,
                    message: format!(
                        "DOCX {} references internal drawing hyperlink relationship '{}' which is not surfaced as a link hint",
                        warning_scope, rel_id
                    ),
                    page: Some(current_page),
                },
            );
            record_scoped_issue_summary(
                scoped_issue_summaries,
                warning_scope_key,
                warning_scope,
                current_page,
                ScopedIssueKind::InternalRelationshipNotSurfaced,
            );
        }
        None => {
            push_unique_warning(
                warnings,
                warning_dedup_keys,
                format!("unresolved_drawing_relationship:{warning_scope_key}:{rel_id}"),
                Warning {
                    kind: WarningKind::UnresolvedRelationship,
                    message: format!(
                        "DOCX {} references drawing hyperlink relationship '{}' which was not found in document relationships",
                        warning_scope, rel_id
                    ),
                    page: Some(current_page),
                },
            );
            record_scoped_issue_summary(
                scoped_issue_summaries,
                warning_scope_key,
                warning_scope,
                current_page,
                ScopedIssueKind::UnresolvedRelationship,
            );
        }
    }
}
