//! Page-level PDF processing: character grouping, path/image extraction, primitive emission.

#[path = "pages/emit.rs"]
mod emit;
#[path = "pages/fonts.rs"]
mod fonts;
#[path = "pages/text/mod.rs"]
mod text;

pub(crate) use emit::paths_to_primitives;
pub(crate) use fonts::{clean_font_name, is_bold_font, is_italic_font, is_non_bold_font};
pub(crate) use text::{
    group_chars_into_spans, infer_document_language_tag, layout_spans_to_text_spans,
    reorder_spans_by_lines,
};

#[cfg(test)]
#[path = "pages/tests.rs"]
mod tests;
