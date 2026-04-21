//! The `extract_spans` fallback: convert pdf_oxide layout spans (Tj/TJ
//! strings) into internal [`TextSpan`]s.
//!
//! Used for page 0 when `extract_chars` + character grouping drops visible
//! text (common on cover pages with mixed fonts / positioning).

use pdf_oxide::layout::FontWeight;
use pdf_oxide::layout::TextSpan as LayoutTextSpan;

use crate::formats::pdf::types::*;
use crate::model::*;

use super::super::fonts::{clean_font_name, is_bold_font, is_italic_font, is_non_bold_font};
use super::direction::infer_text_direction_from_str;

/// Convert pdf_oxide layout spans (Tj/TJ strings) into internal [`TextSpan`]s.
pub(crate) fn layout_spans_to_text_spans(
    layout_spans: Vec<LayoutTextSpan>,
    page_dims: &PageDimensions,
) -> Vec<TextSpan> {
    let mut out = Vec::new();
    for span in layout_spans {
        if let Some(ts) = layout_text_span_to_internal(&span, page_dims) {
            out.push(ts);
        }
    }
    out
}

fn layout_text_span_to_internal(
    span: &LayoutTextSpan,
    page_dims: &PageDimensions,
) -> Option<TextSpan> {
    let content = span.text.trim();
    if content.is_empty() {
        return None;
    }

    let font_name = clean_font_name(&span.font_name);
    let bold_from_name = is_bold_font(&span.font_name);
    let bold_from_weight = matches!(
        span.font_weight,
        FontWeight::Bold | FontWeight::ExtraBold | FontWeight::Black | FontWeight::SemiBold
    );
    let name_says_not_bold = is_non_bold_font(&span.font_name);
    let is_bold = if name_says_not_bold {
        false
    } else {
        bold_from_name || bold_from_weight
    };
    let is_italic = is_italic_font(&span.font_name) || span.is_italic;

    let mut char_bboxes: Vec<(char, BoundingBox)> = Vec::new();
    for ch in span.to_chars() {
        let nb =
            page_dims.normalize_bbox(ch.bbox.x, ch.bbox.y, ch.bbox.width, ch.bbox.height, true);
        char_bboxes.push((ch.char, nb));
    }
    if char_bboxes.is_empty() {
        return None;
    }

    let min_x = char_bboxes
        .iter()
        .map(|(_, b)| b.x)
        .fold(f32::INFINITY, f32::min);
    let max_x = char_bboxes
        .iter()
        .map(|(_, b)| b.x + b.width)
        .fold(f32::NEG_INFINITY, f32::max);
    let min_y = char_bboxes
        .iter()
        .map(|(_, b)| b.y)
        .fold(f32::INFINITY, f32::min);
    let max_y = char_bboxes
        .iter()
        .map(|(_, b)| b.y + b.height)
        .fold(f32::NEG_INFINITY, f32::max);
    let merged_bbox = BoundingBox::new(min_x, min_y, max_x - min_x, max_y - min_y);

    let color = if span.color.r == 0.0 && span.color.g == 0.0 && span.color.b == 0.0 {
        None
    } else {
        Some(Color::rgb(
            (span.color.r * 255.0) as u8,
            (span.color.g * 255.0) as u8,
            (span.color.b * 255.0) as u8,
        ))
    };

    // Detect text direction from character content (same logic as extract_chars path).
    let (text_direction, text_direction_origin) = infer_text_direction_from_str(content);

    Some(TextSpan {
        content: content.to_string(),
        bbox: merged_bbox,
        font_name,
        font_size: span.font_size,
        is_bold,
        is_italic,
        color,
        text_direction,
        text_direction_origin,
        mcid: span.mcid,
        char_bboxes,
    })
}
