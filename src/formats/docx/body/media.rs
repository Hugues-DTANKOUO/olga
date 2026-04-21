use std::collections::{BTreeMap, HashMap, HashSet};

use crate::error::{Warning, WarningKind};
use crate::formats::xml_utils::guess_image_format;
use crate::model::*;
use crate::pipeline::PrimitiveSink;

use super::paragraphs::{attach_docx_media_structural_hints, attach_link_hint};
use super::state::{
    CellFinalizeWarningScope, DocxWarningContext, DrawingContext, ScopedIssueKind,
    ScopedIssueSummary, WarningScopeRef,
};
use super::tables::ActiveTableState;
use super::warnings::{
    docx_warning_scope, docx_warning_scope_location, media_ambiguity_story_scope,
    record_scoped_issue_summary,
};
use super::{ParagraphContext, PartContext};

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_docx_image_for_relationship(
    relationship_id: &str,
    media: &HashMap<String, Vec<u8>>,
    current_drawing: Option<&DrawingContext>,
    para_ctx: Option<&mut ParagraphContext>,
    table_stack: &mut [ActiveTableState],
    primitives: &mut impl PrimitiveSink,
    source_order: &mut u64,
    current_page: u32,
    y_position: &mut f32,
    warnings: &mut Vec<Warning>,
    part_ctx: PartContext,
    current_story_id: Option<&str>,
    in_textbox: bool,
    warning_dedup_keys: &mut HashSet<String>,
    scoped_issue_summaries: &mut BTreeMap<String, ScopedIssueSummary>,
) {
    let Some(data) = media.get(relationship_id) else {
        let (scope_key, warning_scope) =
            docx_warning_scope(table_stack, part_ctx, current_story_id, current_page);
        warnings.push(Warning {
            kind: WarningKind::MissingMedia,
            message: format!(
                "DOCX {} references image relationship '{}' which was not found in media",
                warning_scope, relationship_id
            ),
            page: Some(current_page),
        });
        record_scoped_issue_summary(
            scoped_issue_summaries,
            &scope_key,
            &warning_scope,
            current_page,
            ScopedIssueKind::MissingMedia,
        );
        return;
    };

    let format = guess_image_format(data);
    let bbox = BoundingBox::new(0.1, *y_position, 0.8, 0.1);
    *y_position += 0.1;
    let mut prim = Primitive::reconstructed(
        PrimitiveKind::Image {
            format,
            data: data.clone(),
            alt_text: current_drawing.and_then(|drawing| drawing.alt_text.clone()),
        },
        bbox,
        current_page,
        *source_order,
        GeometrySpace::LogicalPage,
    );
    {
        let mut warning_ctx = DocxWarningContext {
            warnings,
            current_page,
            scoped_issue_summaries,
            warning_dedup_keys,
        };
        attach_docx_media_structural_hints(
            &mut prim,
            part_ctx,
            current_story_id,
            in_textbox,
            table_stack,
            &mut warning_ctx,
        );
    }
    let prim = attach_link_hint(
        prim,
        current_drawing.and_then(|drawing| drawing.hyperlink_url.clone()),
    );
    *source_order += 1;

    let warning_scope =
        docx_warning_scope_location(table_stack, part_ctx, current_story_id, current_page);
    let (summary_scope_key, summary_scope_location) =
        media_ambiguity_story_scope(part_ctx, current_story_id, current_page);
    queue_docx_media_primitive(
        prim,
        para_ctx,
        table_stack,
        primitives,
        warnings,
        current_page,
        scoped_issue_summaries,
        warning_dedup_keys,
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

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_docx_ole_object(
    current_drawing: Option<&DrawingContext>,
    para_ctx: Option<&mut ParagraphContext>,
    table_stack: &mut [ActiveTableState],
    primitives: &mut impl PrimitiveSink,
    source_order: &mut u64,
    current_page: u32,
    y_position: &mut f32,
    warnings: &mut Vec<Warning>,
    part_ctx: PartContext,
    current_story_id: Option<&str>,
    in_textbox: bool,
    warning_dedup_keys: &mut HashSet<String>,
    scoped_issue_summaries: &mut BTreeMap<String, ScopedIssueSummary>,
) {
    let bbox = BoundingBox::new(0.1, *y_position, 0.8, 0.05);
    *y_position += 0.05;
    let mut prim = Primitive::reconstructed(
        PrimitiveKind::Image {
            format: ImageFormat::Unknown,
            data: vec![],
            alt_text: current_drawing
                .and_then(|drawing| drawing.alt_text.clone())
                .or_else(|| Some("[Embedded OLE Object]".to_string())),
        },
        bbox,
        current_page,
        *source_order,
        GeometrySpace::LogicalPage,
    );
    {
        let mut warning_ctx = DocxWarningContext {
            warnings,
            current_page,
            scoped_issue_summaries,
            warning_dedup_keys,
        };
        attach_docx_media_structural_hints(
            &mut prim,
            part_ctx,
            current_story_id,
            in_textbox,
            table_stack,
            &mut warning_ctx,
        );
    }
    let prim = attach_link_hint(
        prim,
        current_drawing.and_then(|drawing| drawing.hyperlink_url.clone()),
    );
    *source_order += 1;

    let (scope_key, warning_scope) =
        docx_warning_scope(table_stack, part_ctx, current_story_id, current_page);
    warnings.push(Warning {
        kind: WarningKind::UnsupportedElement,
        message: format!(
            "DOCX {} contains an embedded OLE object; emitted a placeholder primitive",
            warning_scope
        ),
        page: Some(current_page),
    });
    record_scoped_issue_summary(
        scoped_issue_summaries,
        &scope_key,
        &warning_scope,
        current_page,
        ScopedIssueKind::UnsupportedOlePlaceholder,
    );

    let (summary_scope_key, summary_scope_location) =
        media_ambiguity_story_scope(part_ctx, current_story_id, current_page);
    queue_docx_media_primitive(
        prim,
        para_ctx,
        table_stack,
        primitives,
        warnings,
        current_page,
        scoped_issue_summaries,
        warning_dedup_keys,
        CellFinalizeWarningScope {
            warning: WarningScopeRef {
                key: &scope_key,
                location: &warning_scope,
            },
            summary: WarningScopeRef {
                key: &summary_scope_key,
                location: &summary_scope_location,
            },
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn queue_docx_media_primitive(
    prim: Primitive,
    para_ctx: Option<&mut ParagraphContext>,
    table_stack: &mut [ActiveTableState],
    primitives: &mut impl PrimitiveSink,
    warnings: &mut Vec<Warning>,
    current_page: u32,
    scoped_issue_summaries: &mut BTreeMap<String, ScopedIssueSummary>,
    warning_dedup_keys: &mut HashSet<String>,
    finalize_scope: CellFinalizeWarningScope<'_>,
) {
    if let Some(para) = para_ctx {
        para.pending_media.push(prim);
    } else if let Some(table) = table_stack.last_mut() {
        let mut warning_ctx = DocxWarningContext {
            warnings,
            current_page,
            scoped_issue_summaries,
            warning_dedup_keys,
        };
        table.finalize_current_cell_properties(&mut warning_ctx, finalize_scope);
        table.push_pending_primitive(prim);
    } else {
        primitives.emit(prim);
    }
}
