use std::collections::{BTreeMap, HashMap, HashSet};

use crate::error::{Warning, WarningKind};
use crate::model::*;
use crate::pipeline::PrimitiveSink;

use super::super::super::types::{
    NumberingLevel, ParagraphAlignment, ParagraphContext, PartContext, ResolvedStyle,
};
use super::super::helpers::push_hint_once;
use super::super::state::{
    DocxWarningContext, MediaAmbiguitySummary, ScopedIssueKind, ScopedIssueSummary, WarningScopeRef,
};
use super::super::tables::ActiveTableState;
use super::super::warnings::{
    docx_warning_scope, docx_warning_scope_key, docx_warning_scope_location, push_unique_warning,
    record_media_ambiguity_summary, record_scoped_issue_summary, warn_unresolved_docx_numbering,
};
use super::semantics::resolve_docx_paragraph_container_hints;

pub(in super::super) fn attach_link_hint(
    mut prim: Primitive,
    hyperlink_url: Option<String>,
) -> Primitive {
    if let Some(url) = hyperlink_url {
        push_hint_once(&mut prim, SemanticHint::from_format(HintKind::Link { url }));
    }
    prim
}

pub(in super::super) fn attach_docx_media_structural_hints(
    prim: &mut Primitive,
    part_ctx: PartContext,
    current_story_id: Option<&str>,
    in_textbox: bool,
    table_stack: &[ActiveTableState],
    warning_ctx: &mut DocxWarningContext<'_>,
) {
    match part_ctx {
        PartContext::Header => {
            push_hint_once(prim, SemanticHint::from_format(HintKind::PageHeader));
        }
        PartContext::Footer => {
            push_hint_once(prim, SemanticHint::from_format(HintKind::PageFooter));
        }
        PartContext::Footnotes | PartContext::Endnotes => {
            if let Some(story_id) = current_story_id {
                push_hint_once(
                    prim,
                    SemanticHint::from_format(HintKind::Footnote {
                        id: story_id.to_string(),
                    }),
                );
            } else {
                let (warning_scope_key, warning_scope) = docx_warning_scope(
                    table_stack,
                    part_ctx,
                    current_story_id,
                    warning_ctx.current_page,
                );
                push_unique_warning(
                    warning_ctx.warnings,
                    warning_ctx.warning_dedup_keys,
                    format!("note_media_missing_story_id:{warning_scope_key}"),
                    Warning {
                        kind: WarningKind::PartialExtraction,
                        message: format!(
                            "DOCX {} emitted media inside a footnote/endnote without a story id",
                            warning_scope
                        ),
                        page: Some(warning_ctx.current_page),
                    },
                );
                record_scoped_issue_summary(
                    warning_ctx.scoped_issue_summaries,
                    &warning_scope_key,
                    &warning_scope,
                    warning_ctx.current_page,
                    ScopedIssueKind::NoteMediaMissingStoryId,
                );
            }
        }
        PartContext::Comments => {
            push_hint_once(prim, SemanticHint::from_format(HintKind::Sidebar));
        }
        PartContext::Body => {}
    }

    if in_textbox {
        push_hint_once(prim, SemanticHint::from_format(HintKind::Sidebar));
    }
}

#[allow(clippy::too_many_arguments)]
pub(in super::super) fn emit_pending_paragraph_media(
    para: &mut ParagraphContext,
    styles: &HashMap<String, ResolvedStyle>,
    numbering: &HashMap<(String, u8), NumberingLevel>,
    table_stack: &mut [ActiveTableState],
    primitives: &mut impl PrimitiveSink,
    part_ctx: PartContext,
    current_page: u32,
    warnings: &mut Vec<Warning>,
    warning_dedup_keys: &mut HashSet<String>,
    media_ambiguity_summaries: &mut BTreeMap<String, MediaAmbiguitySummary>,
    scoped_issue_summaries: &mut BTreeMap<String, ScopedIssueSummary>,
) {
    let warning_scope = docx_warning_scope_location(
        table_stack,
        part_ctx,
        para.story_id.as_deref(),
        current_page,
    );
    let warning_scope_key = docx_warning_scope_key(
        table_stack,
        part_ctx,
        para.story_id.as_deref(),
        current_page,
    );
    let has_text = para.runs.iter().any(|run| !run.text.is_empty());
    if !has_text {
        let mut warning_ctx = DocxWarningContext {
            warnings,
            current_page,
            scoped_issue_summaries,
            warning_dedup_keys,
        };
        warn_unresolved_docx_numbering(
            para,
            numbering,
            WarningScopeRef {
                key: &warning_scope_key,
                location: &warning_scope,
            },
            &mut warning_ctx,
        );
    }
    let paragraph_hints = {
        let mut warning_ctx = DocxWarningContext {
            warnings,
            current_page,
            scoped_issue_summaries,
            warning_dedup_keys,
        };
        resolve_docx_paragraph_container_hints(
            para,
            styles,
            numbering,
            &warning_scope_key,
            &warning_scope,
            &mut warning_ctx,
            !has_text,
        )
    };
    for primitive in &mut para.pending_media {
        for hint in &paragraph_hints {
            push_hint_once(primitive, hint.clone());
        }
    }

    let has_annotations = !para.footnote_refs.is_empty() || !para.comment_refs.is_empty();

    if has_annotations {
        match (has_text, para.pending_media.len()) {
            (_, 0) => {}
            (false, 1) => {
                let prim = &mut para.pending_media[0];
                for note_ref in &para.footnote_refs {
                    push_hint_once(
                        prim,
                        SemanticHint::from_format(HintKind::Footnote {
                            id: note_ref.clone(),
                        }),
                    );
                }
                if !para.comment_refs.is_empty() {
                    push_hint_once(prim, SemanticHint::from_format(HintKind::Sidebar));
                }
            }
            _ => {
                push_unique_warning(
                    warnings,
                    warning_dedup_keys,
                    format!("media_annotation_ambiguity:{warning_scope_key}"),
                    Warning {
                        kind: WarningKind::PartialExtraction,
                        message: format!(
                            "DOCX {} paragraph mixes note/comment references with text or multiple media; media-level reference attachment is ambiguous",
                            warning_scope
                        ),
                        page: Some(current_page),
                    },
                );
                record_media_ambiguity_summary(
                    media_ambiguity_summaries,
                    table_stack,
                    part_ctx,
                    para.story_id.as_deref(),
                    current_page,
                );
            }
        }
    }

    for primitive in para.pending_media.drain(..) {
        if let Some(table) = table_stack.last_mut() {
            table.push_pending_primitive(primitive);
        } else {
            primitives.emit(primitive);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(in super::super) fn emit_paragraph(
    para: &ParagraphContext,
    styles: &HashMap<String, ResolvedStyle>,
    numbering: &HashMap<(String, u8), NumberingLevel>,
    table_stack: &mut [ActiveTableState],
    primitives: &mut impl PrimitiveSink,
    source_order: &mut u64,
    current_page: &mut u32,
    y_position: &mut f32,
    part_ctx: PartContext,
    warnings: &mut Vec<Warning>,
    warning_dedup_keys: &mut HashSet<String>,
    scoped_issue_summaries: &mut BTreeMap<String, ScopedIssueSummary>,
) {
    if para.runs.is_empty() {
        return;
    }

    let (warning_scope_key, warning_scope) = docx_warning_scope(
        table_stack,
        part_ctx,
        para.story_id.as_deref(),
        *current_page,
    );

    let indent = para.indent_level as f32 * 0.05;

    let full_text: String = para.runs.iter().map(|r| r.text.as_str()).collect();

    if full_text.is_empty() {
        return;
    }

    // Estimate text width from character count (rough: ~0.006 per char).
    let char_width_estimate = 0.006_f32;
    let est_text_width = (full_text.chars().count() as f32 * char_width_estimate).min(0.95);
    let margin = 0.025;

    let x = match para.alignment {
        ParagraphAlignment::Center => ((1.0 - est_text_width) / 2.0).max(margin),
        ParagraphAlignment::Right => (1.0 - est_text_width - margin).max(margin),
        _ => indent + margin, // Left / Both
    };

    let mut warning_ctx = DocxWarningContext {
        warnings,
        current_page: *current_page,
        scoped_issue_summaries,
        warning_dedup_keys,
    };
    warn_unresolved_docx_numbering(
        para,
        numbering,
        WarningScopeRef {
            key: &warning_scope_key,
            location: &warning_scope,
        },
        &mut warning_ctx,
    );

    let rep_run = &para.runs[0];
    let bbox = BoundingBox::new(x, *y_position, 1.0 - x - 0.05, 0.015);

    let text_direction = if para.is_bidi {
        TextDirection::RightToLeft
    } else {
        rep_run.text_direction
    };
    let text_direction_origin = if para.is_bidi {
        ValueOrigin::Observed
    } else {
        ValueOrigin::Reconstructed
    };

    let lang = para.runs.iter().find_map(|r| r.lang.clone());
    let lang_origin = lang.as_ref().map(|_| ValueOrigin::Observed);

    let mut prim = Primitive::reconstructed(
        PrimitiveKind::Text {
            content: full_text.clone(),
            font_size: rep_run.font_size,
            font_name: rep_run.font_name.clone(),
            is_bold: rep_run.is_bold,
            is_italic: rep_run.is_italic,
            color: rep_run.color,
            char_positions: None,
            text_direction,
            text_direction_origin,
            lang,
            lang_origin,
        },
        bbox,
        *current_page,
        *source_order,
        GeometrySpace::LogicalPage,
    );
    *source_order += 1;

    match part_ctx {
        PartContext::Header => {
            push_hint_once(&mut prim, SemanticHint::from_format(HintKind::PageHeader));
        }
        PartContext::Footer => {
            push_hint_once(&mut prim, SemanticHint::from_format(HintKind::PageFooter));
        }
        PartContext::Footnotes | PartContext::Endnotes => {
            if let Some(story_id) = para.story_id.clone() {
                push_hint_once(
                    &mut prim,
                    SemanticHint::from_format(HintKind::Footnote { id: story_id }),
                );
            } else {
                warnings.push(Warning {
                    kind: WarningKind::PartialExtraction,
                    message: format!(
                        "DOCX {} emitted a note paragraph without a footnote/endnote id",
                        warning_scope
                    ),
                    page: Some(*current_page),
                });
                record_scoped_issue_summary(
                    scoped_issue_summaries,
                    &warning_scope_key,
                    &warning_scope,
                    *current_page,
                    ScopedIssueKind::NoteParagraphMissingStoryId,
                );
            }
        }
        PartContext::Comments => {
            push_hint_once(&mut prim, SemanticHint::from_format(HintKind::Sidebar));
        }
        _ => {}
    }

    if para.in_textbox {
        push_hint_once(&mut prim, SemanticHint::from_format(HintKind::Sidebar));
    }

    let paragraph_hints = {
        let mut warning_ctx = DocxWarningContext {
            warnings,
            current_page: *current_page,
            scoped_issue_summaries,
            warning_dedup_keys,
        };
        resolve_docx_paragraph_container_hints(
            para,
            styles,
            numbering,
            &warning_scope_key,
            &warning_scope,
            &mut warning_ctx,
            true,
        )
    };
    for hint in paragraph_hints {
        push_hint_once(&mut prim, hint);
    }

    for note_ref in &para.footnote_refs {
        push_hint_once(
            &mut prim,
            SemanticHint::from_format(HintKind::Footnote {
                id: note_ref.clone(),
            }),
        );
    }

    if !para.comment_refs.is_empty() {
        push_hint_once(&mut prim, SemanticHint::from_format(HintKind::Sidebar));
    }

    for run in &para.runs {
        if let Some(ref url) = run.hyperlink_url {
            push_hint_once(
                &mut prim,
                SemanticHint::from_format(HintKind::Link { url: url.clone() }),
            );
        }
    }

    if let Some(table) = table_stack.last_mut() {
        table.push_pending_primitive(prim);
    } else {
        primitives.emit(prim);
    }
}
