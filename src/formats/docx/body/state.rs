use std::collections::{BTreeMap, HashSet};

use crate::error::Warning;

use super::super::types::{ParagraphContext, RunContext};
use super::tables::{ActiveTableState, FlatteningSummary};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HeadingLevelOrigin {
    OutlineLevel,
    StyleNameHeuristic,
}

#[derive(Debug, Clone)]
pub(super) struct FlattenedTableContext {
    pub(super) table_id: u64,
    pub(super) row: u32,
    pub(super) col: u32,
    pub(super) depth: u32,
}

#[derive(Debug, Clone)]
pub(super) struct MediaAmbiguitySummary {
    pub(super) page: u32,
    pub(super) location: String,
    pub(super) occurrence_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScopedIssueKind {
    MissingMedia,
    UnresolvedRelationship,
    UnresolvedNumbering,
    UnresolvedStyle,
    HeadingStyleHeuristic,
    MissingReferenceId,
    InternalRelationshipNotSurfaced,
    TrackedDeletionSkipped,
    TrackedInsertionAccepted,
    UnsupportedOlePlaceholder,
    VerticalMergeWithoutRestart,
    VerticalMergeColspanMismatch,
    NoteParagraphMissingStoryId,
    NoteMediaMissingStoryId,
}

#[derive(Debug, Clone)]
pub(super) struct ScopedIssueSummary {
    pub(super) page: u32,
    pub(super) location: String,
    pub(super) issue_kind: ScopedIssueKind,
    pub(super) occurrence_count: u32,
}

pub(super) struct DocxWarningContext<'a> {
    pub(super) warnings: &'a mut Vec<Warning>,
    pub(super) current_page: u32,
    pub(super) scoped_issue_summaries: &'a mut BTreeMap<String, ScopedIssueSummary>,
    pub(super) warning_dedup_keys: &'a mut HashSet<String>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct WarningScopeRef<'a> {
    pub(super) key: &'a str,
    pub(super) location: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CellFinalizeWarningScope<'a> {
    pub(super) warning: WarningScopeRef<'a>,
    pub(super) summary: WarningScopeRef<'a>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct DrawingContext {
    pub(super) alt_text: Option<String>,
    pub(super) hyperlink_url: Option<String>,
}

pub(super) struct BodyParserState {
    pub(super) para_ctx: Option<ParagraphContext>,
    pub(super) run_ctx: Option<RunContext>,
    pub(super) table_stack: Vec<ActiveTableState>,
    pub(super) hyperlink_url: Option<String>,
    pub(super) in_del: bool,
    pub(super) in_sdt_content: bool,
    pub(super) sdt_depth: u32,
    pub(super) textbox_depth: u32,
    pub(super) skip_depth: u32,
    pub(super) in_mc_choice: bool,
    pub(super) current_story_id: Option<String>,
    pub(super) current_drawing: Option<DrawingContext>,
    pub(super) drawing_depth: u32,
    pub(super) next_table_id: u64,
    pub(super) warning_dedup_keys: HashSet<String>,
    pub(super) flattening_summaries: BTreeMap<(u64, u32, u32), FlatteningSummary>,
    pub(super) media_ambiguity_summaries: BTreeMap<String, MediaAmbiguitySummary>,
    pub(super) scoped_issue_summaries: BTreeMap<String, ScopedIssueSummary>,
    pub(super) in_rpr: bool,
    pub(super) in_ppr: bool,
    pub(super) in_trpr: bool,
    pub(super) in_tcpr: bool,
    pub(super) in_t: bool,
}

impl BodyParserState {
    pub(super) fn new() -> Self {
        Self {
            para_ctx: None,
            run_ctx: None,
            table_stack: Vec::new(),
            hyperlink_url: None,
            in_del: false,
            in_sdt_content: false,
            sdt_depth: 0,
            textbox_depth: 0,
            skip_depth: 0,
            in_mc_choice: false,
            current_story_id: None,
            current_drawing: None,
            drawing_depth: 0,
            next_table_id: 0,
            warning_dedup_keys: HashSet::new(),
            flattening_summaries: BTreeMap::new(),
            media_ambiguity_summaries: BTreeMap::new(),
            scoped_issue_summaries: BTreeMap::new(),
            in_rpr: false,
            in_ppr: false,
            in_trpr: false,
            in_tcpr: false,
            in_t: false,
        }
    }
}
