use super::*;

pub(crate) fn handle_end_event<S: PrimitiveSink>(
    e: &BytesEnd<'_>,
    state: &mut BodyParserState,
    ctx: &mut BodyEventContext<'_, S>,
) {
    let name = local_name(e.name().as_ref());

    if state.in_mc_choice {
        if name.as_slice() == b"Choice" && is_mc_element(e.name().as_ref()) {
            state.in_mc_choice = false;
        }
        return;
    }

    match name.as_slice() {
        b"r" if is_w_element(e.name().as_ref()) => {
            if let Some(run) = state.run_ctx.take()
                && let Some(ref mut para) = state.para_ctx
                && !run.text.is_empty()
            {
                para.runs.push(run);
            }
            state.in_rpr = false;
            state.in_t = false;
        }
        b"rPr" => state.in_rpr = false,
        b"t" if is_w_element(e.name().as_ref()) => state.in_t = false,
        b"pPr" => state.in_ppr = false,
        b"trPr" if is_w_element(e.name().as_ref()) => state.in_trpr = false,
        b"tcPr" if is_w_element(e.name().as_ref()) => {
            finalize_active_table_cell_properties(
                &mut state.table_stack,
                ctx.part_ctx,
                state.current_story_id.as_deref(),
                *ctx.current_page,
                ctx.warnings,
                &mut state.scoped_issue_summaries,
                &mut state.warning_dedup_keys,
            );
            state.in_tcpr = false;
        }
        b"p" if is_w_element(e.name().as_ref()) => {
            if let Some(mut para) = state.para_ctx.take() {
                finalize_active_table_cell_properties(
                    &mut state.table_stack,
                    ctx.part_ctx,
                    state.current_story_id.as_deref(),
                    *ctx.current_page,
                    ctx.warnings,
                    &mut state.scoped_issue_summaries,
                    &mut state.warning_dedup_keys,
                );
                emit_pending_paragraph_media(
                    &mut para,
                    ctx.styles,
                    ctx.numbering,
                    &mut state.table_stack,
                    ctx.primitives,
                    ctx.part_ctx,
                    *ctx.current_page,
                    ctx.warnings,
                    &mut state.warning_dedup_keys,
                    &mut state.media_ambiguity_summaries,
                    &mut state.scoped_issue_summaries,
                );
                emit_paragraph(
                    &para,
                    ctx.styles,
                    ctx.numbering,
                    &mut state.table_stack,
                    ctx.primitives,
                    ctx.source_order,
                    ctx.current_page,
                    ctx.y_position,
                    ctx.part_ctx,
                    ctx.warnings,
                    &mut state.warning_dedup_keys,
                    &mut state.scoped_issue_summaries,
                );
                *ctx.y_position += Y_STEP;
            }
        }
        b"tr" if is_w_element(e.name().as_ref()) => {
            if let Some(table) = state.table_stack.last_mut() {
                table.finish_row();
            }
        }
        b"tc" if is_w_element(e.name().as_ref()) => {
            if let Some(table) = state.table_stack.last_mut() {
                table.finish_cell();
            }
        }
        b"tbl" if is_w_element(e.name().as_ref()) => {
            if let Some(table) = state.table_stack.pop() {
                if let Some(parent) = state.table_stack.last_mut() {
                    table.append_into_parent(parent);
                } else {
                    table.flush_pending_primitives(ctx.primitives);
                }
            }
        }
        b"hyperlink" if is_w_element(e.name().as_ref()) => {
            state.hyperlink_url = None;
        }
        b"del" if is_w_element(e.name().as_ref()) => {
            state.in_del = false;
        }
        b"drawing" if is_w_element(e.name().as_ref()) => {
            state.drawing_depth = state.drawing_depth.saturating_sub(1);
            if state.drawing_depth == 0 {
                state.current_drawing = None;
            }
        }
        b"sdtContent" if is_w_element(e.name().as_ref()) => {
            state.sdt_depth = state.sdt_depth.saturating_sub(1);
            if state.sdt_depth == 0 {
                state.in_sdt_content = false;
            }
        }
        b"txbxContent" => {
            state.textbox_depth = state.textbox_depth.saturating_sub(1);
        }
        b"footnote" | b"endnote" | b"comment"
            if matches!(
                ctx.part_ctx,
                PartContext::Footnotes | PartContext::Endnotes | PartContext::Comments
            ) =>
        {
            state.current_story_id = None;
        }
        _ => {}
    }
}
