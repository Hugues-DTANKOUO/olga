use std::collections::{BTreeMap, HashMap, HashSet};

use crate::error::{Warning, WarningKind};

use super::super::types::{NumberingLevel, ParagraphContext, PartContext};
use super::state::{
    DocxWarningContext, MediaAmbiguitySummary, ScopedIssueKind, ScopedIssueSummary, WarningScopeRef,
};
use super::tables::ActiveTableState;
use super::tables::FlatteningSummary;

pub(super) fn docx_story_scope_location(
    part_ctx: PartContext,
    story_id: Option<&str>,
    current_page: u32,
) -> String {
    match part_ctx {
        PartContext::Body => format!("body flow on page {}", current_page),
        PartContext::Header => "header story".to_string(),
        PartContext::Footer => "footer story".to_string(),
        PartContext::Footnotes => format!("footnote story {}", story_id.unwrap_or("unknown")),
        PartContext::Endnotes => format!("endnote story {}", story_id.unwrap_or("unknown")),
        PartContext::Comments => format!("comment story {}", story_id.unwrap_or("unknown")),
    }
}

pub(super) fn docx_warning_scope_location(
    table_stack: &[ActiveTableState],
    part_ctx: PartContext,
    story_id: Option<&str>,
    current_page: u32,
) -> String {
    docx_warning_scope(table_stack, part_ctx, story_id, current_page).1
}

pub(super) fn docx_warning_scope_key(
    table_stack: &[ActiveTableState],
    part_ctx: PartContext,
    story_id: Option<&str>,
    current_page: u32,
) -> String {
    docx_warning_scope(table_stack, part_ctx, story_id, current_page).0
}

pub(super) fn docx_warning_scope(
    table_stack: &[ActiveTableState],
    part_ctx: PartContext,
    story_id: Option<&str>,
    current_page: u32,
) -> (String, String) {
    let (base_key, base_location) = media_ambiguity_story_scope(part_ctx, story_id, current_page);
    table_stack
        .last()
        .and_then(|table| table.current_cell_warning_scope())
        .map(|(cell_key, cell_location)| {
            (
                format!("{}:{}", base_key, cell_key),
                format!("{} {}", base_location, cell_location),
            )
        })
        .unwrap_or((base_key, base_location))
}

pub(super) fn warn_unresolved_docx_numbering(
    para: &ParagraphContext,
    numbering: &HashMap<(String, u8), NumberingLevel>,
    warning_scope: WarningScopeRef<'_>,
    warning_ctx: &mut DocxWarningContext<'_>,
) {
    let Some(num_id) = &para.num_id else {
        return;
    };

    let level = para.num_level.unwrap_or(0);
    if numbering.contains_key(&(num_id.clone(), level)) {
        return;
    }

    push_unique_warning(
        warning_ctx.warnings,
        warning_ctx.warning_dedup_keys,
        format!(
            "unresolved_numbering:{}:{num_id}:{level}",
            warning_scope.key
        ),
        Warning {
            kind: WarningKind::PartialExtraction,
            message: format!(
                "DOCX {} paragraph references unresolved numbering definition '{}'/level {}",
                warning_scope.location, num_id, level
            ),
            page: Some(warning_ctx.current_page),
        },
    );
    record_scoped_issue_summary(
        warning_ctx.scoped_issue_summaries,
        warning_scope.key,
        warning_scope.location,
        warning_ctx.current_page,
        ScopedIssueKind::UnresolvedNumbering,
    );
}

pub(super) fn record_media_ambiguity_summary(
    summaries: &mut BTreeMap<String, MediaAmbiguitySummary>,
    table_stack: &[ActiveTableState],
    part_ctx: PartContext,
    story_id: Option<&str>,
    current_page: u32,
) {
    let (base_key, base_location) = media_ambiguity_story_scope(part_ctx, story_id, current_page);
    let (key, location) = table_stack
        .last()
        .and_then(|table| table.current_cell_warning_scope())
        .map(|(cell_key, cell_location)| {
            (
                format!("{}:{}", base_key, cell_key),
                format!("{} {}", base_location, cell_location),
            )
        })
        .unwrap_or((base_key, base_location));

    let summary = summaries.entry(key).or_insert(MediaAmbiguitySummary {
        page: current_page,
        location,
        occurrence_count: 0,
    });
    summary.occurrence_count += 1;
}

pub(super) fn record_scoped_issue_summary(
    summaries: &mut BTreeMap<String, ScopedIssueSummary>,
    scope_key: &str,
    scope_location: &str,
    current_page: u32,
    issue_kind: ScopedIssueKind,
) {
    let key = format!("{}:{}", scope_key, issue_kind.key_suffix());
    let summary = summaries.entry(key).or_insert(ScopedIssueSummary {
        page: current_page,
        location: scope_location.to_string(),
        issue_kind,
        occurrence_count: 0,
    });
    summary.occurrence_count += 1;
}

pub(super) fn media_ambiguity_story_scope(
    part_ctx: PartContext,
    story_id: Option<&str>,
    current_page: u32,
) -> (String, String) {
    match part_ctx {
        PartContext::Body => (
            format!("body:page:{}", current_page),
            format!("body flow on page {}", current_page),
        ),
        PartContext::Header => ("header".to_string(), "header story".to_string()),
        PartContext::Footer => ("footer".to_string(), "footer story".to_string()),
        PartContext::Footnotes => (
            format!("footnote:{}", story_id.unwrap_or("unknown")),
            format!("footnote story {}", story_id.unwrap_or("unknown")),
        ),
        PartContext::Endnotes => (
            format!("endnote:{}", story_id.unwrap_or("unknown")),
            format!("endnote story {}", story_id.unwrap_or("unknown")),
        ),
        PartContext::Comments => (
            format!("comment:{}", story_id.unwrap_or("unknown")),
            format!("comment story {}", story_id.unwrap_or("unknown")),
        ),
    }
}

impl ScopedIssueKind {
    pub(super) fn key_suffix(self) -> &'static str {
        match self {
            Self::MissingMedia => "missing_media",
            Self::UnresolvedRelationship => "unresolved_relationship",
            Self::UnresolvedNumbering => "unresolved_numbering",
            Self::UnresolvedStyle => "unresolved_style",
            Self::HeadingStyleHeuristic => "heading_style_heuristic",
            Self::MissingReferenceId => "missing_reference_id",
            Self::InternalRelationshipNotSurfaced => "internal_relationship_not_surfaced",
            Self::TrackedDeletionSkipped => "tracked_deletion_skipped",
            Self::TrackedInsertionAccepted => "tracked_insertion_accepted",
            Self::UnsupportedOlePlaceholder => "unsupported_ole_placeholder",
            Self::VerticalMergeWithoutRestart => "vertical_merge_without_restart",
            Self::VerticalMergeColspanMismatch => "vertical_merge_colspan_mismatch",
            Self::NoteParagraphMissingStoryId => "note_paragraph_missing_story_id",
            Self::NoteMediaMissingStoryId => "note_media_missing_story_id",
        }
    }

    pub(super) fn summary_message(self, location: &str, occurrence_count: u32) -> String {
        match self {
            Self::MissingMedia => format!(
                "DOCX {} contains {} missing media references; some image targets could not be loaded",
                location, occurrence_count
            ),
            Self::UnresolvedRelationship => format!(
                "DOCX {} contains {} unresolved relationships; some hyperlink or drawing targets could not be resolved",
                location, occurrence_count
            ),
            Self::UnresolvedNumbering => format!(
                "DOCX {} contains {} paragraphs referencing unresolved numbering definitions",
                location, occurrence_count
            ),
            Self::UnresolvedStyle => format!(
                "DOCX {} contains {} paragraphs referencing unknown styles",
                location, occurrence_count
            ),
            Self::HeadingStyleHeuristic => format!(
                "DOCX {} contains {} paragraphs promoted to headings via style-name heuristic",
                location, occurrence_count
            ),
            Self::MissingReferenceId => format!(
                "DOCX {} contains {} malformed note/comment references missing w:id",
                location, occurrence_count
            ),
            Self::InternalRelationshipNotSurfaced => format!(
                "DOCX {} contains {} internal hyperlink relationships that are not surfaced as link hints",
                location, occurrence_count
            ),
            Self::TrackedDeletionSkipped => format!(
                "DOCX {} contains {} skipped deletions from tracked changes",
                location, occurrence_count
            ),
            Self::TrackedInsertionAccepted => format!(
                "DOCX {} contains {} accepted insertions from tracked changes",
                location, occurrence_count
            ),
            Self::UnsupportedOlePlaceholder => format!(
                "DOCX {} contains {} embedded OLE objects emitted as placeholder primitives",
                location, occurrence_count
            ),
            Self::VerticalMergeWithoutRestart => format!(
                "DOCX {} contains {} vertical merge continuations without a matching restart cell",
                location, occurrence_count
            ),
            Self::VerticalMergeColspanMismatch => format!(
                "DOCX {} contains {} vertical merge continuations whose colspan differs from the restart cell",
                location, occurrence_count
            ),
            Self::NoteParagraphMissingStoryId => format!(
                "DOCX {} contains {} note paragraphs emitted without a footnote/endnote id",
                location, occurrence_count
            ),
            Self::NoteMediaMissingStoryId => format!(
                "DOCX {} contains {} note media items emitted without a footnote/endnote id",
                location, occurrence_count
            ),
        }
    }
}

pub(super) fn push_unique_warning(
    warnings: &mut Vec<Warning>,
    dedup_keys: &mut HashSet<String>,
    dedup_key: String,
    warning: Warning,
) {
    if dedup_keys.insert(dedup_key) {
        warnings.push(warning);
    }
}

pub(super) fn emit_flattening_summary_warnings(
    summaries: &BTreeMap<(u64, u32, u32), FlatteningSummary>,
    warnings: &mut Vec<Warning>,
) {
    for summary in summaries.values() {
        if summary.nested_table_count > 1 || summary.max_depth > 1 {
            warnings.push(Warning {
                kind: WarningKind::PartialExtraction,
                message: format!(
                    "DOCX {} required flattening of {} nested tables up to depth {}; inner cell semantics are preserved, but full table containment is lost in the Primitive layer",
                    summary.location, summary.nested_table_count, summary.max_depth
                ),
                page: Some(summary.page),
            });
        }
    }
}

pub(super) fn emit_media_ambiguity_summary_warnings(
    summaries: &BTreeMap<String, MediaAmbiguitySummary>,
    warnings: &mut Vec<Warning>,
) {
    for summary in summaries.values() {
        if summary.occurrence_count > 1 {
            warnings.push(Warning {
                kind: WarningKind::PartialExtraction,
                message: format!(
                    "DOCX {} contains {} paragraphs where media-level note/comment attachment is ambiguous",
                    summary.location, summary.occurrence_count
                ),
                page: Some(summary.page),
            });
        }
    }
}

pub(super) fn emit_scoped_issue_summary_warnings(
    summaries: &BTreeMap<String, ScopedIssueSummary>,
    warnings: &mut Vec<Warning>,
) {
    for summary in summaries.values() {
        if summary.occurrence_count > 1 {
            warnings.push(Warning {
                kind: match summary.issue_kind {
                    ScopedIssueKind::MissingMedia => WarningKind::MissingMedia,
                    ScopedIssueKind::UnresolvedRelationship => WarningKind::UnresolvedRelationship,
                    ScopedIssueKind::UnresolvedNumbering => WarningKind::PartialExtraction,
                    ScopedIssueKind::UnresolvedStyle => WarningKind::UnresolvedStyle,
                    ScopedIssueKind::HeadingStyleHeuristic => WarningKind::HeuristicInference,
                    ScopedIssueKind::MissingReferenceId => WarningKind::MalformedContent,
                    ScopedIssueKind::InternalRelationshipNotSurfaced => {
                        WarningKind::PartialExtraction
                    }
                    ScopedIssueKind::VerticalMergeWithoutRestart
                    | ScopedIssueKind::VerticalMergeColspanMismatch
                    | ScopedIssueKind::NoteParagraphMissingStoryId
                    | ScopedIssueKind::NoteMediaMissingStoryId => WarningKind::PartialExtraction,
                    ScopedIssueKind::TrackedDeletionSkipped
                    | ScopedIssueKind::TrackedInsertionAccepted => {
                        WarningKind::TrackedChangeResolved
                    }
                    ScopedIssueKind::UnsupportedOlePlaceholder => WarningKind::UnsupportedElement,
                },
                message: summary
                    .issue_kind
                    .summary_message(&summary.location, summary.occurrence_count),
                page: Some(summary.page),
            });
        }
    }
}
