use super::*;
use crate::model::{BoundingBox, HintKind, Primitive, PrimitiveKind, SemanticHint, TextDirection};

/// Create a text primitive at a specific (x, y) position with given width.
fn text_at(x: f32, y: f32, width: f32, content: &str, order: u64) -> Primitive {
    Primitive::observed(
        PrimitiveKind::Text {
            content: content.to_string(),
            font_size: 12.0,
            font_name: "Arial".to_string(),
            is_bold: false,
            is_italic: false,
            color: None,
            char_positions: None,
            text_direction: Default::default(),
            text_direction_origin: crate::model::provenance::ValueOrigin::Observed,
            lang: None,
            lang_origin: None,
        },
        BoundingBox::new(x, y, width, 0.02),
        0,
        order,
    )
}

/// Create a text primitive with a specific text direction.
fn text_at_dir(
    x: f32,
    y: f32,
    width: f32,
    content: &str,
    order: u64,
    dir: TextDirection,
) -> Primitive {
    Primitive::observed(
        PrimitiveKind::Text {
            content: content.to_string(),
            font_size: 12.0,
            font_name: "Arial".to_string(),
            is_bold: false,
            is_italic: false,
            color: None,
            char_positions: None,
            text_direction: dir,
            text_direction_origin: crate::model::provenance::ValueOrigin::Observed,
            lang: None,
            lang_origin: None,
        },
        BoundingBox::new(x, y, width, 0.02),
        0,
        order,
    )
}

/// Create a text primitive with a format-derived hint.
fn hinted_text(x: f32, y: f32, content: &str, order: u64) -> Primitive {
    text_at(x, y, 0.4, content, order).with_hint(SemanticHint::from_format(HintKind::Paragraph))
}

/// Create a line primitive at a specific position.
fn line_at(x: f32, y: f32, width: f32, order: u64) -> Primitive {
    use crate::model::Point;
    Primitive::observed(
        PrimitiveKind::Line {
            start: Point::new(x, y),
            end: Point::new(x + width, y),
            thickness: 0.001,
            color: None,
        },
        BoundingBox::new(x, y, width, 0.001),
        0,
        order,
    )
}

/// Extract text content from a primitive (for assertion readability).
fn texts(primitives: &[Primitive]) -> Vec<&str> {
    primitives.iter().filter_map(|p| p.text_content()).collect()
}

#[path = "tests/columns.rs"]
mod columns;
#[path = "tests/edge_cases.rs"]
mod edge_cases;
#[path = "tests/selection.rs"]
mod selection;
#[path = "tests/simple.rs"]
mod simple;
#[path = "tests/tagged.rs"]
mod tagged;
