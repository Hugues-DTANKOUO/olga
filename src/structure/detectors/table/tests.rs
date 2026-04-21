use super::*;
use crate::model::{BoundingBox, Color, Point, Primitive, PrimitiveKind};
use crate::structure::detectors::DetectionContext;
use crate::structure::types::FontHistogram;

mod benchmarks;
mod headers;
mod lattice;
mod stream;
mod validation;

fn text_at(x: f32, y: f32, content: &str, order: u64) -> Primitive {
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
        BoundingBox::new(x, y, 0.15, 0.02),
        0,
        order,
    )
}

fn h_line(y: f32, x_start: f32, x_end: f32, order: u64) -> Primitive {
    Primitive::observed(
        PrimitiveKind::Line {
            start: Point::new(x_start, y),
            end: Point::new(x_end, y),
            thickness: 0.001,
            color: None,
        },
        BoundingBox::new(x_start, y, x_end - x_start, 0.001),
        0,
        order,
    )
}

fn v_line(x: f32, y_start: f32, y_end: f32, order: u64) -> Primitive {
    Primitive::observed(
        PrimitiveKind::Line {
            start: Point::new(x, y_start),
            end: Point::new(x, y_end),
            thickness: 0.001,
            color: None,
        },
        BoundingBox::new(x, y_start, 0.001, y_end - y_start),
        0,
        order,
    )
}

fn ctx() -> DetectionContext {
    DetectionContext {
        font_histogram: FontHistogram::default(),
        page: 0,
        page_dims: None,
        text_boxes: None,
        open_table_columns: None,
    }
}
