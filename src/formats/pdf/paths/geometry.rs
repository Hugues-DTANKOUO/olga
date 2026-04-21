use std::collections::HashMap;

use pdf_oxide::content::Matrix;
use pdf_oxide::geometry::{Point as PdfPoint, Rect as PdfRect};
use pdf_oxide::object::Object;

use super::*;

pub(super) fn raw_paths_match(left: &RawPath, right: &RawPath) -> bool {
    match (left, right) {
        (
            RawPath::Line {
                x0: lx0,
                y0: ly0,
                x1: lx1,
                y1: ly1,
                ..
            },
            RawPath::Line {
                x0: rx0,
                y0: ry0,
                x1: rx1,
                y1: ry1,
                ..
            },
        ) => {
            approx_eq_f32(
                lx0.min(*lx1),
                rx0.min(*rx1),
                PATH_ARTIFACT_MATCH_TOLERANCE_PT,
            ) && approx_eq_f32(
                lx0.max(*lx1),
                rx0.max(*rx1),
                PATH_ARTIFACT_MATCH_TOLERANCE_PT,
            ) && approx_eq_f32(
                ly0.min(*ly1),
                ry0.min(*ry1),
                PATH_ARTIFACT_MATCH_TOLERANCE_PT,
            ) && approx_eq_f32(
                ly0.max(*ly1),
                ry0.max(*ry1),
                PATH_ARTIFACT_MATCH_TOLERANCE_PT,
            )
        }
        (
            RawPath::Rect {
                x: lx,
                y: ly,
                width: lw,
                height: lh,
                ..
            },
            RawPath::Rect {
                x: rx,
                y: ry,
                width: rw,
                height: rh,
                ..
            },
        ) => {
            approx_eq_f32(*lx, *rx, PATH_ARTIFACT_MATCH_TOLERANCE_PT)
                && approx_eq_f32(*ly, *ry, PATH_ARTIFACT_MATCH_TOLERANCE_PT)
                && approx_eq_f32(*lw, *rw, PATH_ARTIFACT_MATCH_TOLERANCE_PT)
                && approx_eq_f32(*lh, *rh, PATH_ARTIFACT_MATCH_TOLERANCE_PT)
        }
        _ => false,
    }
}

pub(super) fn approx_eq_f32(left: f32, right: f32, tolerance: f32) -> bool {
    (left - right).abs() <= tolerance
}

pub(super) fn axis_aligned_rectangle_bbox(commands: &[ArtifactPathCommand]) -> Option<PdfRect> {
    let points = match commands {
        [
            ArtifactPathCommand::MoveTo(p0),
            ArtifactPathCommand::LineTo(p1),
            ArtifactPathCommand::LineTo(p2),
            ArtifactPathCommand::LineTo(p3),
        ]
        | [
            ArtifactPathCommand::MoveTo(p0),
            ArtifactPathCommand::LineTo(p1),
            ArtifactPathCommand::LineTo(p2),
            ArtifactPathCommand::LineTo(p3),
            ArtifactPathCommand::ClosePath,
        ] => [*p0, *p1, *p2, *p3],
        _ => return None,
    };

    let tolerance = 0.1;
    let side1 = approx_eq_f32(points[0].x, points[1].x, tolerance)
        || approx_eq_f32(points[0].y, points[1].y, tolerance);
    let side2 = approx_eq_f32(points[1].x, points[2].x, tolerance)
        || approx_eq_f32(points[1].y, points[2].y, tolerance);
    let side3 = approx_eq_f32(points[2].x, points[3].x, tolerance)
        || approx_eq_f32(points[2].y, points[3].y, tolerance);

    if !(side1 && side2 && side3) {
        return None;
    }

    let min_x = points
        .iter()
        .map(|point| point.x)
        .fold(f32::INFINITY, f32::min);
    let max_x = points
        .iter()
        .map(|point| point.x)
        .fold(f32::NEG_INFINITY, f32::max);
    let min_y = points
        .iter()
        .map(|point| point.y)
        .fold(f32::INFINITY, f32::min);
    let max_y = points
        .iter()
        .map(|point| point.y)
        .fold(f32::NEG_INFINITY, f32::max);

    Some(PdfRect::new(min_x, min_y, max_x - min_x, max_y - min_y))
}

pub(super) fn transform_pdf_point(matrix: Matrix, x: f32, y: f32) -> PdfPoint {
    matrix.transform_point(x, y)
}

pub(super) fn transform_pdf_rect(
    matrix: Matrix,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> PdfRect {
    let corners = [
        matrix.transform_point(x, y),
        matrix.transform_point(x + width, y),
        matrix.transform_point(x, y + height),
        matrix.transform_point(x + width, y + height),
    ];

    let min_x = corners
        .iter()
        .map(|point| point.x)
        .fold(f32::INFINITY, f32::min);
    let max_x = corners
        .iter()
        .map(|point| point.x)
        .fold(f32::NEG_INFINITY, f32::max);
    let min_y = corners
        .iter()
        .map(|point| point.y)
        .fold(f32::INFINITY, f32::min);
    let max_y = corners
        .iter()
        .map(|point| point.y)
        .fold(f32::NEG_INFINITY, f32::max);

    PdfRect::new(min_x, min_y, max_x - min_x, max_y - min_y)
}

pub(super) fn extract_form_xobject_matrix(xobject_dict: &HashMap<String, Object>) -> Matrix {
    let Some(matrix_obj) = xobject_dict.get("Matrix") else {
        return Matrix::identity();
    };
    let Some(array) = matrix_obj.as_array() else {
        return Matrix::identity();
    };
    if array.len() < 6 {
        return Matrix::identity();
    }

    let mut values = [0.0f32; 6];
    for (idx, value) in array.iter().take(6).enumerate() {
        let number = value
            .as_real()
            .map(|value| value as f32)
            .or_else(|| value.as_integer().map(|value| value as f32));
        let Some(number) = number else {
            return Matrix::identity();
        };
        values[idx] = number;
    }

    Matrix {
        a: values[0],
        b: values[1],
        c: values[2],
        d: values[3],
        e: values[4],
        f: values[5],
    }
}

pub(super) fn pdf_rgb_to_u8(color: (f32, f32, f32)) -> (u8, u8, u8) {
    (
        (color.0 * 255.0) as u8,
        (color.1 * 255.0) as u8,
        (color.2 * 255.0) as u8,
    )
}
