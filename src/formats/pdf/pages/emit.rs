use crate::error::Warning;
use crate::model::*;
use crate::pipeline::PrimitiveSink;

use super::super::types::*;

/// Convert raw paths into Primitives (Lines and Rectangles).
pub(crate) fn paths_to_primitives(
    paths: &[RawPath],
    page_dims: &PageDimensions,
    page: u32,
    source_order: &mut u64,
    _warnings: &mut Vec<Warning>,
    sink: &mut impl PrimitiveSink,
) {
    for path in paths {
        match path {
            RawPath::Line {
                x0,
                y0,
                x1,
                y1,
                thickness,
                color_r,
                color_g,
                color_b,
            } => {
                let start = page_dims.normalize_point(*x0, *y0, true);
                let end = page_dims.normalize_point(*x1, *y1, true);

                let norm_thickness = thickness / page_dims.effective_width.max(1.0);

                let bbox = BoundingBox::new(
                    start.x.min(end.x),
                    start.y.min(end.y),
                    (start.x - end.x).abs().max(norm_thickness),
                    (start.y - end.y).abs().max(norm_thickness),
                );

                let prim = Primitive::observed(
                    PrimitiveKind::Line {
                        start,
                        end,
                        thickness: norm_thickness,
                        color: Some(Color::rgb(*color_r, *color_g, *color_b)),
                    },
                    bbox,
                    page,
                    *source_order,
                );
                *source_order += 1;
                sink.emit(prim);
            }
            RawPath::Rect {
                x,
                y,
                width,
                height,
                fill_r,
                fill_g,
                fill_b,
                stroke_r,
                stroke_g,
                stroke_b,
                stroke_thickness,
            } => {
                let bbox = page_dims.normalize_bbox(*x, *y, *width, *height, true);

                let fill = match (fill_r, fill_g, fill_b) {
                    (Some(r), Some(g), Some(b)) => Some(Color::rgb(*r, *g, *b)),
                    _ => None,
                };
                let stroke = match (stroke_r, stroke_g, stroke_b) {
                    (Some(r), Some(g), Some(b)) => Some(Color::rgb(*r, *g, *b)),
                    _ => None,
                };
                let norm_st = Some(stroke_thickness / page_dims.effective_width.max(1.0));

                let prim = Primitive::observed(
                    PrimitiveKind::Rectangle {
                        fill,
                        stroke,
                        stroke_thickness: norm_st,
                    },
                    bbox,
                    page,
                    *source_order,
                );
                *source_order += 1;
                sink.emit(prim);
            }
        }
    }
}
