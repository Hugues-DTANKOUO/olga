//! Raw-char to [`TextSpan`] grouping pipeline (the `extract_chars` path).
//!
//! Given per-glyph [`RawChar`]s emitted by pdf_oxide, cluster them into
//! visual lines, merge runs with consistent font/format into spans, and
//! synthesize inter-word spaces from geometric gaps.

use crate::formats::pdf::types::*;
use crate::model::*;

use super::clustering::cluster_chars_into_lines;
use super::direction::{
    infer_span_text_direction, inter_char_gap, is_vertical_rotation, ordered_chars_for_span,
    synthesize_space_bbox,
};

/// Group raw characters into text spans.
///
/// Algorithm (v2):
/// 1. Sort characters by position (top-to-bottom, left-to-right) so we don't
///    depend on PDF emission order, which is not guaranteed.
/// 2. For each character, check if it belongs to the current span:
///    - Same font properties (name, size, bold, italic)
///    - Horizontal proximity using advance_width (preferred) or bbox gap (fallback)
///    - Vertical alignment within LINE_DRIFT_RATIO of font_size
/// 3. Insert inter-word spaces when the gap exceeds WORD_GAP_RATIO
/// 4. Propagate dominant color into the span
///
/// References:
/// - PDFMiner LAParams: char_margin=2.0, word_margin=0.1, line_overlap=0.5
/// - PdfPig: 20% Manhattan distance threshold
/// - pdf_oxide advance_width: typographic metric from font Widths dictionary
pub(crate) fn group_chars_into_spans(
    chars: &[RawChar],
    page_dims: &PageDimensions,
) -> Vec<TextSpan> {
    if chars.is_empty() {
        return Vec::new();
    }

    // Step 1: Sort by position to ensure correct processing order.
    // PDF content streams do NOT guarantee characters arrive in reading order.
    //
    // We cannot simply sort by (y, x): on skewed scans (e.g. slightly rotated
    // documents) chars on the same visual line have slightly different raw_y,
    // and a pure y-sort reverses reading order. Instead we cluster chars into
    // visual lines based on vertical overlap, order the lines by baseline, and
    // sort by x within each line.
    let sorted_chars: Vec<&RawChar> = cluster_chars_into_lines(chars);

    let mut spans: Vec<TextSpan> = Vec::new();
    let mut current_chars: Vec<&RawChar> = Vec::new();

    for ch in &sorted_chars {
        if ch.text.is_whitespace() && ch.raw_width <= 0.0 {
            if !current_chars.is_empty() {
                // Keep the current span intact; the next visible character will
                // reconstruct a spacing gap if needed.
            }
            continue;
        }

        let should_break = if let Some(prev) = current_chars.last() {
            if prev.font_name != ch.font_name
                || (prev.font_size - ch.font_size).abs() > 0.5
                || prev.is_bold != ch.is_bold
                || prev.is_italic != ch.is_italic
                || prev.mcid != ch.mcid
            {
                true
            } else {
                let font_size = prev.font_size.max(1.0);
                let prev_vertical = is_vertical_rotation(prev.rotation_degrees);
                let curr_vertical = is_vertical_rotation(ch.rotation_degrees);

                if prev_vertical != curr_vertical {
                    true
                } else if prev_vertical {
                    let axis_gap = ((ch.raw_y - prev.raw_y).abs()
                        - prev.raw_height.max(ch.raw_height))
                    .max(0.0);
                    let cross_drift = (ch.raw_x - prev.raw_x).abs();

                    axis_gap > font_size * SPAN_BREAK_GAP_RATIO
                        || cross_drift > font_size * LINE_DRIFT_RATIO
                } else {
                    let h_gap = if prev.advance_width > 0.0 {
                        ch.origin_x - (prev.origin_x + prev.advance_width)
                    } else {
                        ch.raw_x - (prev.raw_x + prev.raw_width)
                    };

                    let v_drift = (ch.raw_y - prev.raw_y).abs();

                    h_gap > font_size * SPAN_BREAK_GAP_RATIO
                        || v_drift > font_size * LINE_DRIFT_RATIO
                        || ch.raw_x < prev.raw_x - font_size * NEWLINE_BACKTRACK_RATIO
                }
            }
        } else {
            false
        };

        if should_break && !current_chars.is_empty() {
            if let Some(span) = flush_span(&current_chars, page_dims) {
                spans.push(span);
            }
            current_chars.clear();
        }

        current_chars.push(ch);
    }

    if !current_chars.is_empty()
        && let Some(span) = flush_span(&current_chars, page_dims)
    {
        spans.push(span);
    }

    spans
}

fn flush_span(chars: &[&RawChar], page_dims: &PageDimensions) -> Option<TextSpan> {
    if chars.is_empty() {
        return None;
    }

    let (text_direction, text_direction_origin) = infer_span_text_direction(chars);
    let ordered_chars = ordered_chars_for_span(chars, text_direction);
    let mut text = String::new();
    let mut char_bboxes = Vec::new();

    for (i, ch) in ordered_chars.iter().enumerate() {
        if i > 0 {
            let prev = ordered_chars[i - 1];
            let font_size = prev.font_size.max(1.0);
            let gap = inter_char_gap(prev, ch, text_direction);

            // Synthesize an inter-word space only when neither side is
            // already whitespace. Justified PDFs combine a real space glyph
            // with a large TJ displacement for inter-word stretching, which
            // would otherwise double-count: once as the real space, once as
            // a geometry-driven synthetic space. Suppressing the synthetic
            // on either adjacency keeps single spaces single and lets the
            // real whitespace represent the break.
            let already_spaced = prev.text.is_whitespace() || ch.text.is_whitespace();
            if gap > font_size * WORD_GAP_RATIO && !already_spaced {
                text.push(' ');
                let space_bbox = synthesize_space_bbox(prev, ch, gap, text_direction, page_dims);
                char_bboxes.push((' ', space_bbox));
            }
        }

        let char_bbox =
            page_dims.normalize_bbox(ch.raw_x, ch.raw_y, ch.raw_width, ch.raw_height, true);
        // Push the character directly — NFC normalization of a single codepoint
        // from pdf_oxide is a no-op in practice (combining marks are not emitted
        // as standalone RawChars by the PDF text extraction pipeline).
        text.push(ch.text);
        char_bboxes.push((ch.text, char_bbox));
    }

    let trimmed = text.trim().to_string();
    if trimmed.is_empty() {
        return None;
    }

    let leading_spaces = text.chars().take_while(|ch| ch.is_whitespace()).count();
    let trailing_spaces = text
        .chars()
        .rev()
        .take_while(|ch| ch.is_whitespace())
        .count();
    if leading_spaces > 0 || trailing_spaces > 0 {
        let end = char_bboxes.len() - trailing_spaces;
        char_bboxes = char_bboxes[leading_spaces..end].to_vec();
    }

    let min_x = chars.iter().map(|c| c.raw_x).fold(f32::INFINITY, f32::min);
    let max_x = chars
        .iter()
        .map(|c| c.raw_x + c.raw_width)
        .fold(f32::NEG_INFINITY, f32::max);
    let min_y = chars.iter().map(|c| c.raw_y).fold(f32::INFINITY, f32::min);
    let max_y = chars
        .iter()
        .map(|c| c.raw_y + c.raw_height)
        .fold(f32::NEG_INFINITY, f32::max);

    let merged_bbox = page_dims.normalize_bbox(min_x, min_y, max_x - min_x, max_y - min_y, true);

    let first = ordered_chars.first()?;
    let color = compute_span_color(chars);

    Some(TextSpan {
        content: trimmed,
        bbox: merged_bbox,
        font_name: first.font_name.clone(),
        font_size: first.font_size,
        is_bold: first.is_bold,
        is_italic: first.is_italic,
        color,
        text_direction,
        text_direction_origin,
        mcid: first.mcid,
        char_bboxes,
    })
}

fn compute_span_color(chars: &[&RawChar]) -> Option<Color> {
    if chars.is_empty() {
        return None;
    }

    let first = chars[0];
    let r = (first.color_r * 255.0) as u8;
    let g = (first.color_g * 255.0) as u8;
    let b = (first.color_b * 255.0) as u8;

    if r == 0 && g == 0 && b == 0 {
        None
    } else {
        Some(Color::rgb(r, g, b))
    }
}
