use super::*;

pub(crate) fn handle_empty_event<S: PrimitiveSink>(
    e: &BytesStart<'_>,
    state: &mut BodyParserState,
    ctx: &mut BodyEventContext<'_, S>,
) {
    let name = local_name(e.name().as_ref());

    if state.in_del || state.in_mc_choice {
        return;
    }

    if state.in_ppr
        && let Some(ref mut para) = state.para_ctx
    {
        apply_docx_paragraph_property(para, name.as_slice(), e);
    }

    if state.in_trpr
        && name.as_slice() == b"tblHeader"
        && is_w_element(e.name().as_ref())
        && bool_attr(e, true)
        && let Some(table) = state.table_stack.last_mut()
    {
        table.note_header_row(true);
    }

    if state.in_tcpr && is_w_element(e.name().as_ref()) {
        apply_docx_table_property(&mut state.table_stack, name.as_slice(), e);
    }

    if state.drawing_depth > 0 {
        match name.as_slice() {
            b"docPr" => {
                handle_docx_drawing_alt_text(state.current_drawing.as_mut(), e);
            }
            b"hlinkClick" => {
                handle_docx_drawing_hyperlink(
                    state.current_drawing.as_mut(),
                    e,
                    ctx.rels,
                    &state.table_stack,
                    ctx.part_ctx,
                    state.current_story_id.as_deref(),
                    *ctx.current_page,
                    ctx.warnings,
                    &mut state.scoped_issue_summaries,
                    &mut state.warning_dedup_keys,
                );
            }
            _ => {}
        }
    }

    if state.in_rpr
        && let Some(ref mut run) = state.run_ctx
    {
        apply_docx_run_property(run, name.as_slice(), e);
    }

    handle_page_break(name.as_slice(), e, ctx.current_page, ctx.y_position);

    if let Some(ref mut para) = state.para_ctx {
        handle_docx_reference(
            para,
            name.as_slice(),
            e,
            &state.table_stack,
            ctx.part_ctx,
            state.current_story_id.as_deref(),
            *ctx.current_page,
            ctx.warnings,
            &mut state.scoped_issue_summaries,
        );
    }

    if name.as_slice() == b"blip" {
        for attr in e.attributes().flatten() {
            let k = local_name(attr.key.as_ref());
            if k == b"embed" {
                emit_docx_image_for_relationship(
                    &attr_value(&attr),
                    ctx.media,
                    state.current_drawing.as_ref(),
                    state.para_ctx.as_mut(),
                    &mut state.table_stack,
                    ctx.primitives,
                    ctx.source_order,
                    *ctx.current_page,
                    ctx.y_position,
                    ctx.warnings,
                    ctx.part_ctx,
                    state.current_story_id.as_deref(),
                    state.textbox_depth > 0,
                    &mut state.warning_dedup_keys,
                    &mut state.scoped_issue_summaries,
                );
            }
        }
    }

    if name.as_slice() == b"OLEObject" || name.as_slice() == b"oleObj" {
        emit_docx_ole_object(
            state.current_drawing.as_ref(),
            state.para_ctx.as_mut(),
            &mut state.table_stack,
            ctx.primitives,
            ctx.source_order,
            *ctx.current_page,
            ctx.y_position,
            ctx.warnings,
            ctx.part_ctx,
            state.current_story_id.as_deref(),
            state.textbox_depth > 0,
            &mut state.warning_dedup_keys,
            &mut state.scoped_issue_summaries,
        );
    }
}
