use pdf_oxide::elements::PathContent;
use pdf_oxide::geometry::{Point as PdfPoint, Rect as PdfRect};

use crate::error::{Warning, WarningKind};

use super::geometry::{axis_aligned_rectangle_bbox, pdf_rgb_to_u8, raw_paths_match};
use super::*;

pub(super) fn filter_artifact_paths(
    paths: Vec<RawPath>,
    artifact_candidates: Option<&[ArtifactPathCandidate]>,
    page: u32,
    warnings: &mut Vec<Warning>,
) -> Vec<RawPath> {
    let Some(artifact_candidates) = artifact_candidates else {
        return paths;
    };

    if artifact_candidates.is_empty() {
        return paths;
    }

    let mut used_candidates = vec![false; artifact_candidates.len()];
    let mut filtered_artifact_total = 0usize;
    let mut filtered_artifact_counts = BTreeMap::new();
    let filtered = paths
        .into_iter()
        .filter(|path| {
            let match_idx = artifact_candidates
                .iter()
                .enumerate()
                .find_map(|(idx, candidate)| {
                    (!used_candidates[idx] && raw_paths_match(path, &candidate.path)).then_some(idx)
                });

            if let Some(idx) = match_idx {
                used_candidates[idx] = true;
                filtered_artifact_total += 1;
                *filtered_artifact_counts
                    .entry(describe_artifact_kind(
                        artifact_candidates[idx].artifact_type.as_ref(),
                    ))
                    .or_insert(0usize) += 1;
                false
            } else {
                true
            }
        })
        .collect::<Vec<_>>();

    if filtered_artifact_total > 0 {
        let details = filtered_artifact_counts
            .into_iter()
            .map(|(label, count)| format!("{} {}", count, label))
            .collect::<Vec<_>>()
            .join(", ");
        warnings.push(Warning {
            kind: WarningKind::FilteredArtifact,
            message: format!(
                "Filtered {} PDF vector artifact(s) on this page ({})",
                filtered_artifact_total, details
            ),
            page: Some(page),
        });
    }

    filtered
}

pub(super) fn convert_paths(
    paths: &[PathContent],
    _page: u32,
    _warnings: &mut Vec<Warning>,
) -> Vec<RawPath> {
    paths
        .iter()
        .filter_map(|path| {
            let bbox = &path.bbox;
            let (sr, sg, sb) = extract_stroke_color(path);
            let w = bbox.width.abs();
            let h = bbox.height.abs();

            // Classification by bbox aspect ratio.
            // This is the mathematical truth: regardless of how many PDF
            // operations compose the path, its bounding box tells us what it IS.
            //
            // A path whose bbox has an extreme aspect ratio is a segment:
            //   min(w,h) / max(w,h) < 0.1 → segment (line)
            //   Otherwise → rectangle
            //
            // This correctly handles table borders encoded as thin filled
            // rectangles with 6 operations (move_to + multiple line_to),
            // which the old code skipped as "complex paths".

            let max_dim = w.max(h);
            let min_dim = w.min(h);

            // Skip invisible paths (both dimensions negligible).
            if max_dim < 0.5 {
                return None;
            }

            // Segment detection: extreme aspect ratio.
            if min_dim / max_dim.max(0.001) < 0.1 || min_dim < LINE_TOLERANCE_PT {
                return Some(classify_as_line(bbox, path.stroke_width, sr, sg, sb));
            }

            // Everything else is a rectangle.
            Some(classify_as_rect(path, bbox, sr, sg, sb))
        })
        .collect()
}

pub(in crate::formats::pdf) fn build_artifact_path_candidate(
    builder: &ArtifactPathBuilder,
    graphics_state: &pdf_oxide::content::GraphicsState,
    paint_mode: PaintMode,
) -> Option<RawPath> {
    if builder.has_complex_segments {
        return None;
    }

    if let Some(bbox) = axis_aligned_rectangle_bbox(&builder.commands) {
        if bbox.width.abs() < LINE_TOLERANCE_PT || bbox.height.abs() < LINE_TOLERANCE_PT {
            return paint_mode.stroke.then(|| {
                classify_artifact_line(
                    PdfPoint {
                        x: bbox.x,
                        y: bbox.y,
                    },
                    PdfPoint {
                        x: bbox.x + bbox.width,
                        y: bbox.y + bbox.height,
                    },
                    graphics_state.line_width,
                    graphics_state.stroke_color_rgb,
                )
            });
        }

        return Some(classify_artifact_rect(
            &bbox,
            paint_mode,
            graphics_state.stroke_color_rgb,
            graphics_state.fill_color_rgb,
            graphics_state.line_width,
        ));
    }

    match builder.commands.as_slice() {
        [
            ArtifactPathCommand::MoveTo(start),
            ArtifactPathCommand::LineTo(end),
        ]
        | [
            ArtifactPathCommand::MoveTo(start),
            ArtifactPathCommand::LineTo(end),
            ArtifactPathCommand::ClosePath,
        ] if paint_mode.stroke => Some(classify_artifact_line(
            *start,
            *end,
            graphics_state.line_width,
            graphics_state.stroke_color_rgb,
        )),
        [ArtifactPathCommand::Rectangle(bbox)]
        | [
            ArtifactPathCommand::Rectangle(bbox),
            ArtifactPathCommand::ClosePath,
        ] => {
            if bbox.width.abs() < LINE_TOLERANCE_PT || bbox.height.abs() < LINE_TOLERANCE_PT {
                paint_mode.stroke.then(|| {
                    classify_artifact_line(
                        PdfPoint {
                            x: bbox.x,
                            y: bbox.y,
                        },
                        PdfPoint {
                            x: bbox.x + bbox.width,
                            y: bbox.y + bbox.height,
                        },
                        graphics_state.line_width,
                        graphics_state.stroke_color_rgb,
                    )
                })
            } else {
                Some(classify_artifact_rect(
                    bbox,
                    paint_mode,
                    graphics_state.stroke_color_rgb,
                    graphics_state.fill_color_rgb,
                    graphics_state.line_width,
                ))
            }
        }
        _ => None,
    }
}

fn extract_stroke_color(path: &PathContent) -> (u8, u8, u8) {
    if let Some(ref c) = path.stroke_color {
        (
            (c.r * 255.0) as u8,
            (c.g * 255.0) as u8,
            (c.b * 255.0) as u8,
        )
    } else {
        (0, 0, 0)
    }
}

fn classify_as_line(
    bbox: &pdf_oxide::geometry::Rect,
    stroke_width: f32,
    sr: u8,
    sg: u8,
    sb: u8,
) -> RawPath {
    RawPath::Line {
        x0: bbox.x,
        y0: bbox.y,
        x1: bbox.x + bbox.width,
        y1: bbox.y + bbox.height,
        thickness: bbox
            .width
            .abs()
            .min(bbox.height.abs())
            .max(stroke_width)
            .max(0.5),
        color_r: sr,
        color_g: sg,
        color_b: sb,
    }
}

fn classify_as_rect(
    path: &PathContent,
    bbox: &pdf_oxide::geometry::Rect,
    sr: u8,
    sg: u8,
    sb: u8,
) -> RawPath {
    let fill = path.fill_color.as_ref().map(|c| {
        (
            (c.r * 255.0) as u8,
            (c.g * 255.0) as u8,
            (c.b * 255.0) as u8,
        )
    });

    RawPath::Rect {
        x: bbox.x,
        y: bbox.y,
        width: bbox.width.abs(),
        height: bbox.height.abs(),
        fill_r: fill.map(|f| f.0),
        fill_g: fill.map(|f| f.1),
        fill_b: fill.map(|f| f.2),
        stroke_r: Some(sr),
        stroke_g: Some(sg),
        stroke_b: Some(sb),
        stroke_thickness: path.stroke_width,
    }
}

fn classify_artifact_line(
    start: PdfPoint,
    end: PdfPoint,
    line_width: f32,
    stroke_rgb: (f32, f32, f32),
) -> RawPath {
    let x0 = start.x.min(end.x);
    let y0 = start.y.min(end.y);
    let x1 = start.x.max(end.x);
    let y1 = start.y.max(end.y);
    let (color_r, color_g, color_b) = pdf_rgb_to_u8(stroke_rgb);

    RawPath::Line {
        x0,
        y0,
        x1,
        y1,
        thickness: (x1 - x0)
            .abs()
            .min((y1 - y0).abs())
            .max(line_width)
            .max(0.5),
        color_r,
        color_g,
        color_b,
    }
}

fn classify_artifact_rect(
    bbox: &PdfRect,
    paint_mode: PaintMode,
    stroke_rgb: (f32, f32, f32),
    fill_rgb: (f32, f32, f32),
    line_width: f32,
) -> RawPath {
    let (stroke_r, stroke_g, stroke_b) = pdf_rgb_to_u8(stroke_rgb);
    let (fill_r, fill_g, fill_b) = pdf_rgb_to_u8(fill_rgb);

    RawPath::Rect {
        x: bbox.x,
        y: bbox.y,
        width: bbox.width.abs(),
        height: bbox.height.abs(),
        fill_r: paint_mode.fill.then_some(fill_r),
        fill_g: paint_mode.fill.then_some(fill_g),
        fill_b: paint_mode.fill.then_some(fill_b),
        stroke_r: paint_mode.stroke.then_some(stroke_r),
        stroke_g: paint_mode.stroke.then_some(stroke_g),
        stroke_b: paint_mode.stroke.then_some(stroke_b),
        stroke_thickness: line_width,
    }
}
