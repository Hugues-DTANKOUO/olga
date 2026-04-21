use super::*;
use crate::model::{
    BoundingBox, HintKind, ImageFormat, NodeKind, Primitive, PrimitiveKind, SemanticHint,
};
use crate::structure::types::PageFlush;

fn text_prim(y: f32, content: &str, order: u64) -> Primitive {
    text_prim_geo(0.05, y, 0.9, 12.0, content, 0, order)
}

fn text_prim_geo(
    x: f32,
    y: f32,
    width: f32,
    font_size: f32,
    content: &str,
    page: u32,
    order: u64,
) -> Primitive {
    Primitive::observed(
        PrimitiveKind::Text {
            content: content.to_string(),
            font_size,
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
        page,
        order,
    )
}

fn image_prim(y: f32, alt: Option<&str>, order: u64) -> Primitive {
    Primitive::observed(
        PrimitiveKind::Image {
            format: ImageFormat::Png,
            data: vec![0x89, 0x50, 0x4E, 0x47],
            alt_text: alt.map(|s| s.to_string()),
        },
        BoundingBox::new(0.1, y, 0.5, 0.3),
        0,
        order,
    )
}

fn line_prim(y: f32, order: u64) -> Primitive {
    Primitive::observed(
        PrimitiveKind::Line {
            start: crate::model::Point { x: 0.05, y },
            end: crate::model::Point { x: 0.95, y },
            thickness: 0.001,
            color: None,
        },
        BoundingBox::new(0.05, y, 0.9, 0.001),
        0,
        order,
    )
}

fn rect_prim(y: f32, order: u64) -> Primitive {
    Primitive::observed(
        PrimitiveKind::Rectangle {
            fill: Some(crate::model::Color::white()),
            stroke: None,
            stroke_thickness: None,
        },
        BoundingBox::new(0.0, y, 1.0, 0.05),
        0,
        order,
    )
}

fn hinted(prim: Primitive, kind: HintKind) -> Primitive {
    prim.with_hint(SemanticHint::from_format(kind))
}

fn hinted_conf(prim: Primitive, kind: HintKind, confidence: f32) -> Primitive {
    prim.with_hint(SemanticHint::from_heuristic(
        kind,
        confidence,
        "test-heuristic",
    ))
}

fn page_flush(page: u32, primitives: Vec<Primitive>) -> PageFlush {
    PageFlush {
        page,
        primitives,
        page_dims: None,
        visual_lines: None,
    }
}

#[path = "tests/content.rs"]
mod content;
#[path = "tests/continuity.rs"]
mod continuity;
#[path = "tests/core.rs"]
mod core;
#[path = "tests/table_continuation.rs"]
mod table_continuation;
#[path = "tests/warnings.rs"]
mod warnings;
