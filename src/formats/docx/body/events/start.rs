use super::*;

pub(crate) fn handle_start_event<S: PrimitiveSink>(
    e: &BytesStart<'_>,
    state: &mut BodyParserState,
    ctx: &mut BodyEventContext<'_, S>,
) {
    let name = local_name(e.name().as_ref());

    if state.in_del {
        return;
    }

    if handle_docx_mc_choice_start(
        name.as_slice(),
        e.name().as_ref(),
        &mut state.in_mc_choice,
        &mut state.skip_depth,
    ) {
        return;
    }

    match name.as_slice() {
        b"p" if is_w_element(e.name().as_ref()) => {
            state.para_ctx = Some(start_docx_paragraph_context(
                state.current_story_id.as_deref(),
                state.textbox_depth > 0,
            ));
        }
        b"pPr" if state.para_ctx.is_some() => {
            state.in_ppr = true;
        }
        b"r" if is_w_element(e.name().as_ref()) && state.para_ctx.is_some() => {
            state.run_ctx = state.para_ctx.as_ref().map(|para| {
                start_docx_run_context(para, ctx.styles, state.hyperlink_url.as_deref())
            });
        }
        b"rPr" if state.run_ctx.is_some() => {
            state.in_rpr = true;
        }
        b"t" if is_w_element(e.name().as_ref()) && state.run_ctx.is_some() => {
            state.in_t = true;
        }
        b"tbl" if is_w_element(e.name().as_ref()) => {
            handle_docx_table_start(
                &mut state.table_stack,
                &mut state.flattening_summaries,
                ctx.warnings,
                &mut state.warning_dedup_keys,
                ctx.part_ctx,
                state.current_story_id.as_deref(),
                *ctx.current_page,
                &mut state.next_table_id,
            );
        }
        b"tr" if is_w_element(e.name().as_ref()) => {
            if let Some(table) = state.table_stack.last_mut() {
                table.start_row();
            }
        }
        b"trPr" if is_w_element(e.name().as_ref()) => {
            state.in_trpr = true;
        }
        b"tblHeader" if state.in_trpr && is_w_element(e.name().as_ref()) => {
            if let Some(table) = state.table_stack.last_mut() {
                table.note_header_row(bool_attr(e, true));
            }
        }
        b"tc" if is_w_element(e.name().as_ref()) => {
            if let Some(table) = state.table_stack.last_mut() {
                table.start_cell();
            }
        }
        b"tcPr" if is_w_element(e.name().as_ref()) => {
            state.in_tcpr = true;
        }
        b"gridSpan" if state.in_tcpr && is_w_element(e.name().as_ref()) => {
            for attr in e.attributes().flatten() {
                if local_name(attr.key.as_ref()) == b"val"
                    && let Ok(span) = attr_value(&attr).parse::<u32>()
                    && let Some(table) = state.table_stack.last_mut()
                {
                    table.set_current_colspan(span);
                }
            }
        }
        b"vMerge" if state.in_tcpr && is_w_element(e.name().as_ref()) => {
            apply_docx_table_property(&mut state.table_stack, name.as_slice(), e);
        }
        b"hyperlink" if is_w_element(e.name().as_ref()) => {
            handle_docx_hyperlink_start(
                e,
                ctx.rels,
                &state.table_stack,
                ctx.part_ctx,
                state.current_story_id.as_deref(),
                *ctx.current_page,
                &mut state.hyperlink_url,
                ctx.warnings,
                &mut state.scoped_issue_summaries,
            );
        }
        b"del" | b"ins" => handle_docx_tracked_change_start(
            name.as_slice(),
            e.name().as_ref(),
            &state.table_stack,
            ctx.part_ctx,
            state.current_story_id.as_deref(),
            *ctx.current_page,
            &mut state.in_del,
            ctx.warnings,
            &mut state.scoped_issue_summaries,
        ),
        b"sdtContent" if is_w_element(e.name().as_ref()) => {
            state.in_sdt_content = true;
            state.sdt_depth += 1;
        }
        b"txbxContent" => {
            state.textbox_depth += 1;
        }
        b"footnote" | b"endnote" | b"comment"
            if matches!(
                ctx.part_ctx,
                PartContext::Footnotes | PartContext::Endnotes | PartContext::Comments
            ) =>
        {
            state.current_story_id = story_id_from_element(e);
        }
        b"drawing" if is_w_element(e.name().as_ref()) => {
            state.drawing_depth += 1;
            if state.drawing_depth == 1 {
                state.current_drawing = Some(super::super::state::DrawingContext {
                    hyperlink_url: state.hyperlink_url.clone(),
                    ..Default::default()
                });
            }
        }
        b"docPr" if state.drawing_depth > 0 => {
            handle_docx_drawing_alt_text(state.current_drawing.as_mut(), e);
        }
        b"hlinkClick" if state.drawing_depth > 0 => {
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
