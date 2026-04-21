use super::traversal::build_tagged_pdf_context;
use super::*;
use crate::formats::pdf::types::TextSpan;
use crate::model::{BoundingBox, Color};
use crate::model::{HintKind, SemanticHint, TextDirection, ValueOrigin};
use pdf_oxide::object::Object;
use pdf_oxide::structure::{StructChild, StructElem, StructTreeRoot, StructType};

fn dummy_span(mcid: Option<u32>, text: &str) -> TextSpan {
    TextSpan {
        content: text.to_string(),
        bbox: BoundingBox::new(0.0, 0.0, 0.1, 0.1),
        font_name: "Helvetica".to_string(),
        font_size: 12.0,
        is_bold: false,
        is_italic: false,
        color: Some(Color::black()),
        text_direction: TextDirection::LeftToRight,
        text_direction_origin: ValueOrigin::Observed,
        mcid,
        char_bboxes: Vec::new(),
    }
}

#[path = "tests/apply.rs"]
mod apply;
#[path = "tests/context.rs"]
mod context;
#[path = "tests/images.rs"]
mod images;
#[path = "tests/structure.rs"]
mod structure;
