use pdf_oxide::PdfDocument;
use pdf_oxide::elements::PathContent;

use crate::error::Warning;
use crate::model::{PageDimensions, Primitive};

use super::super::artifact_risk::ArtifactRiskCollector;
use super::super::pages::paths_to_primitives;
use super::super::paths::{convert_paths, filter_artifact_paths, scan_page_artifact_paths};
use super::super::resources::CachedPageData;
use super::super::tagged::TaggedPdfContext;
use super::super::types::RawPath;

pub(super) struct PagePathDecodeContext<'a> {
    pub doc: &'a mut PdfDocument,
    pub page_idx: usize,
    pub page_num: u32,
    pub page_dims: &'a PageDimensions,
    pub tagged_context: &'a TaggedPdfContext,
    pub cached_page: Option<&'a CachedPageData>,
    pub artifact_risk: &'a mut ArtifactRiskCollector,
    pub source_order: &'a mut u64,
    pub warnings: &'a mut Vec<Warning>,
    pub primitives: &'a mut Vec<Primitive>,
}

pub(super) fn extract_page_paths(ctx: PagePathDecodeContext<'_>) {
    let PagePathDecodeContext {
        doc,
        page_idx,
        page_num,
        page_dims,
        tagged_context,
        cached_page,
        artifact_risk,
        source_order,
        warnings,
        primitives,
    } = ctx;

    let cached_res = cached_page.map(|p| &p.resources);
    let artifact_paths = scan_page_artifact_paths(doc, page_idx, warnings, cached_res);

    let raw_paths_from_extract = match doc.extract_paths(page_idx) {
        Ok(paths) => {
            let converted = convert_paths(&paths, page_num, warnings);
            filter_artifact_paths(converted, artifact_paths.as_deref(), page_num, warnings)
        }
        Err(_) => Vec::new(),
    };

    let mut extra_raw_paths = Vec::new();
    if let Ok(rects) = doc.extract_rects(page_idx) {
        for rect in &rects {
            let b = &rect.bbox;
            let (sr, sg, sb) = extract_stroke_color_from_path(rect);
            let (fr, fg, fb) = extract_fill_color_from_path(rect);
            extra_raw_paths.push(RawPath::Rect {
                x: b.x,
                y: b.y,
                width: b.width,
                height: b.height,
                fill_r: fr,
                fill_g: fg,
                fill_b: fb,
                stroke_r: sr,
                stroke_g: sg,
                stroke_b: sb,
                stroke_thickness: rect.stroke_width,
            });
        }
    }
    if let Ok(lines) = doc.extract_lines(page_idx) {
        for line in &lines {
            let b = &line.bbox;
            let (sr, sg, sb) = extract_stroke_color_from_path(line);
            extra_raw_paths.push(RawPath::Line {
                x0: b.x,
                y0: b.y,
                x1: b.x + b.width,
                y1: b.y + b.height,
                thickness: line.stroke_width,
                color_r: sr.unwrap_or(0),
                color_g: sg.unwrap_or(0),
                color_b: sb.unwrap_or(0),
            });
        }
    }

    let merged = merge_raw_paths(raw_paths_from_extract, extra_raw_paths);

    let page_ctx = tagged_context.page_context(page_num);
    artifact_risk.note_paths(
        &merged,
        page_dims,
        page_num,
        tagged_context.use_structure_reading_order(),
        page_ctx,
    );
    paths_to_primitives(
        &merged,
        page_dims,
        page_num,
        source_order,
        warnings,
        primitives,
    );
}

fn extract_stroke_color_from_path(path: &PathContent) -> (Option<u8>, Option<u8>, Option<u8>) {
    if let Some(ref c) = path.stroke_color {
        (
            Some((c.r * 255.0) as u8),
            Some((c.g * 255.0) as u8),
            Some((c.b * 255.0) as u8),
        )
    } else {
        (None, None, None)
    }
}

fn extract_fill_color_from_path(path: &PathContent) -> (Option<u8>, Option<u8>, Option<u8>) {
    if let Some(ref c) = path.fill_color {
        (
            Some((c.r * 255.0) as u8),
            Some((c.g * 255.0) as u8),
            Some((c.b * 255.0) as u8),
        )
    } else {
        (None, None, None)
    }
}

fn merge_raw_paths(primary: Vec<RawPath>, extra: Vec<RawPath>) -> Vec<RawPath> {
    if extra.is_empty() {
        return primary;
    }
    if primary.is_empty() {
        return extra;
    }

    fn path_bbox(p: &RawPath) -> (f32, f32, f32, f32) {
        match p {
            RawPath::Line { x0, y0, x1, y1, .. } => {
                (x0.min(*x1), y0.min(*y1), x0.max(*x1), y0.max(*y1))
            }
            RawPath::Rect {
                x,
                y,
                width,
                height,
                ..
            } => (*x, *y, x + width, y + height),
        }
    }

    let mut primary_bboxes: Vec<(f32, f32, f32, f32)> = primary.iter().map(path_bbox).collect();
    primary_bboxes.sort_by(|a, b| a.1.total_cmp(&b.1));

    let mut merged = primary;

    for extra_path in extra {
        let eb = path_bbox(&extra_path);
        let extra_area = ((eb.2 - eb.0) * (eb.3 - eb.1)).max(0.001);

        let overlaps = primary_bboxes.iter().any(|pb| {
            if pb.1 > eb.3 {
                return false;
            }
            if pb.3 < eb.1 {
                return false;
            }
            let overlap_x = (pb.2.min(eb.2) - pb.0.max(eb.0)).max(0.0);
            let overlap_y = (pb.3.min(eb.3) - pb.1.max(eb.1)).max(0.0);
            let overlap_area = overlap_x * overlap_y;
            overlap_area / extra_area > 0.5
        });

        if !overlaps {
            merged.push(extra_path);
        }
    }

    merged
}
