#[path = "helpers/context.rs"]
mod context;
#[path = "helpers/events.rs"]
mod events;

pub(super) use context::{
    apply_docx_paragraph_property, apply_docx_run_property, apply_docx_table_property,
    push_hint_once, start_docx_paragraph_context, start_docx_run_context, story_id_from_element,
};
pub(super) use events::{
    finalize_active_table_cell_properties, handle_docx_drawing_alt_text,
    handle_docx_drawing_hyperlink, handle_docx_hyperlink_start, handle_docx_mc_choice_start,
    handle_docx_reference, handle_docx_table_start, handle_docx_tracked_change_start,
    handle_page_break,
};
