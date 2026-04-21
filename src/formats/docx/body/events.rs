#[path = "events/empty.rs"]
mod empty;
#[path = "events/end.rs"]
mod end;
#[path = "events/start.rs"]
mod start;

use std::collections::HashMap;

use quick_xml::events::{BytesEnd, BytesStart, BytesText};
use unicode_normalization::UnicodeNormalization;

use crate::error::{Warning, WarningKind};
use crate::formats::xml_utils::*;
use crate::pipeline::PrimitiveSink;

use super::super::types::*;
use super::helpers::{
    apply_docx_paragraph_property, apply_docx_run_property, apply_docx_table_property,
    finalize_active_table_cell_properties, handle_docx_drawing_alt_text,
    handle_docx_drawing_hyperlink, handle_docx_hyperlink_start, handle_docx_mc_choice_start,
    handle_docx_reference, handle_docx_table_start, handle_docx_tracked_change_start,
    handle_page_break, start_docx_paragraph_context, start_docx_run_context, story_id_from_element,
};
use super::media::{emit_docx_image_for_relationship, emit_docx_ole_object};
use super::paragraphs::{emit_paragraph, emit_pending_paragraph_media};
use super::state::BodyParserState;
use super::warnings::docx_warning_scope_location;

pub(super) use empty::handle_empty_event;
pub(super) use end::handle_end_event;
pub(super) use start::handle_start_event;

pub(super) const Y_STEP: f32 = 0.02;

pub(super) struct BodyEventContext<'a, S: PrimitiveSink> {
    pub(super) styles: &'a HashMap<String, ResolvedStyle>,
    pub(super) numbering: &'a HashMap<(String, u8), NumberingLevel>,
    pub(super) rels: &'a HashMap<String, Relationship>,
    pub(super) media: &'a HashMap<String, Vec<u8>>,
    pub(super) primitives: &'a mut S,
    pub(super) source_order: &'a mut u64,
    pub(super) current_page: &'a mut u32,
    pub(super) y_position: &'a mut f32,
    pub(super) warnings: &'a mut Vec<Warning>,
    pub(super) part_ctx: PartContext,
}

pub(super) fn handle_text_event(e: &BytesText<'_>, state: &mut BodyParserState) {
    if !state.in_t || state.in_del || state.in_mc_choice {
        return;
    }

    if let Some(ref mut run) = state.run_ctx {
        let text = String::from_utf8_lossy(e.as_ref()).to_string();
        let normalized: String = text.nfc().collect();
        run.text.push_str(&normalized);
    }
}

/// Handle an `Event::GeneralRef` — entity references like `&apos;`, `&amp;`, etc.
///
/// In quick-xml 0.38+/0.39+, predefined XML entities are no longer inlined into
/// `Event::Text`.  Instead they arrive as separate `Event::GeneralRef` events.
/// We resolve them here and append the character to the current run's text
/// accumulator, exactly as `handle_text_event` does for plain text.
pub(super) fn handle_general_ref_event(
    e: &quick_xml::events::BytesRef<'_>,
    state: &mut BodyParserState,
) {
    if !state.in_t || state.in_del || state.in_mc_choice {
        return;
    }

    if let Some(ref mut run) = state.run_ctx {
        // Try character references first (&#39; &#x27; etc.)
        if let Ok(Some(ch)) = e.resolve_char_ref() {
            run.text.push(ch);
            return;
        }
        // Then try predefined named entities (&apos; &amp; &lt; &gt; &quot;).
        let name = String::from_utf8_lossy(e.as_ref());
        if let Some(ch) = resolve_predefined_entity(&name) {
            run.text.push(ch);
        }
        // Unknown entities are silently dropped — DOCX should not contain them.
    }
}

pub(super) fn handle_xml_error(
    state: &BodyParserState,
    current_page: u32,
    part_ctx: PartContext,
    warnings: &mut Vec<Warning>,
    error: quick_xml::Error,
) {
    let warning_scope = docx_warning_scope_location(
        &state.table_stack,
        part_ctx,
        state.current_story_id.as_deref(),
        current_page,
    );
    warnings.push(Warning {
        kind: WarningKind::UnexpectedStructure,
        message: format!("DOCX {} XML parse error: {}", warning_scope, error),
        page: Some(current_page),
    });
}
