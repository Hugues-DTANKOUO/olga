//! DOCX body XML streaming parser.
//!
//! This is the core parsing loop that streams through `word/document.xml` (and
//! secondary parts like headers/footers) to emit [`Primitive`]s with [`SemanticHint`]s.

#[path = "body/events.rs"]
mod events;
#[path = "body/helpers.rs"]
mod helpers;
#[path = "body/media.rs"]
mod media;
#[path = "body/paragraphs.rs"]
mod paragraphs;
#[path = "body/state.rs"]
mod state;
#[path = "body/tables.rs"]
mod tables;
#[path = "body/warnings.rs"]
mod warnings;

use std::collections::HashMap;

use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::error::Warning;
use crate::pipeline::PrimitiveSink;

use self::events::{
    BodyEventContext, handle_empty_event, handle_end_event, handle_general_ref_event,
    handle_start_event, handle_text_event, handle_xml_error,
};
use self::state::BodyParserState;
use super::types::*;
use warnings::{
    emit_flattening_summary_warnings, emit_media_ambiguity_summary_warnings,
    emit_scoped_issue_summary_warnings,
};

// ---------------------------------------------------------------------------
// Body XML parsing (the main loop)
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub(crate) fn parse_body_xml(
    xml: &[u8],
    styles: &HashMap<String, ResolvedStyle>,
    numbering: &HashMap<(String, u8), NumberingLevel>,
    rels: &HashMap<String, Relationship>,
    media: &HashMap<String, Vec<u8>>,
    primitives: &mut impl PrimitiveSink,
    source_order: &mut u64,
    current_page: &mut u32,
    y_position: &mut f32,
    warnings: &mut Vec<Warning>,
    part_ctx: PartContext,
) -> bool {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut state = BodyParserState::new();
    let mut had_xml_error = false;

    loop {
        let mut event_ctx = BodyEventContext {
            styles,
            numbering,
            rels,
            media,
            primitives,
            source_order,
            current_page,
            y_position,
            warnings,
            part_ctx,
        };

        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => handle_start_event(e, &mut state, &mut event_ctx),
            Ok(Event::Empty(ref e)) => handle_empty_event(e, &mut state, &mut event_ctx),
            Ok(Event::Text(ref e)) => handle_text_event(e, &mut state),
            Ok(Event::GeneralRef(ref e)) => handle_general_ref_event(e, &mut state),
            Ok(Event::End(ref e)) => handle_end_event(e, &mut state, &mut event_ctx),
            Ok(Event::Eof) => break,
            Err(e) => {
                had_xml_error = true;
                handle_xml_error(
                    &state,
                    *event_ctx.current_page,
                    part_ctx,
                    event_ctx.warnings,
                    e,
                );
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    emit_flattening_summary_warnings(&state.flattening_summaries, warnings);
    emit_media_ambiguity_summary_warnings(&state.media_ambiguity_summaries, warnings);
    emit_scoped_issue_summary_warnings(&state.scoped_issue_summaries, warnings);

    had_xml_error
}
