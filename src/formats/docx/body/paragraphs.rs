#[path = "paragraphs/drawing.rs"]
mod drawing;
#[path = "paragraphs/emit.rs"]
mod emit;
#[path = "paragraphs/semantics.rs"]
mod semantics;

pub(super) use drawing::{capture_docx_drawing_alt_text, capture_docx_drawing_link};
pub(super) use emit::{
    attach_docx_media_structural_hints, attach_link_hint, emit_paragraph,
    emit_pending_paragraph_media,
};
