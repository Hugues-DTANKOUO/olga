//! Text-direction inference and direction-dependent per-char geometry helpers.
//!
//! These are shared between the `extract_chars` grouping path (via
//! [`grouping`](super::grouping)) and the `extract_spans` fallback (via
//! [`layout_path`](super::layout_path)).

use std::cmp::Ordering;

use pdf_oxide::text::rtl_detector::is_rtl_text;

use crate::formats::pdf::types::*;
use crate::model::*;

/// A rotation_degrees value considered "vertical" (90° or 270° ±20°).
///
/// Used to distinguish CJK column text from horizontal text at the glyph
/// level. The ±20° slack absorbs small scan skew while still excluding
/// 45° stamps/watermarks.
pub(super) fn is_vertical_rotation(rotation_degrees: f32) -> bool {
    let normalized = rotation_degrees.rem_euclid(360.0);
    (normalized - 90.0).abs() <= 20.0 || (normalized - 270.0).abs() <= 20.0
}

/// Infer text direction from a string (used by the extract_spans path where
/// we don't have per-character RawChar structs).
pub(super) fn infer_text_direction_from_str(text: &str) -> (TextDirection, ValueOrigin) {
    let mut rtl_chars = 0usize;
    let mut ltr_chars = 0usize;
    for ch in text.chars() {
        if is_rtl_text(ch as u32) {
            rtl_chars += 1;
        } else if ch.is_alphabetic() {
            ltr_chars += 1;
        }
    }
    let direction = match (rtl_chars > 0, ltr_chars > 0) {
        (true, true) => TextDirection::Mixed,
        (true, false) => TextDirection::RightToLeft,
        _ => TextDirection::LeftToRight,
    };
    let origin = if rtl_chars > 0 || ltr_chars > 0 {
        ValueOrigin::Reconstructed
    } else {
        ValueOrigin::Estimated
    };
    (direction, origin)
}

pub(super) fn infer_span_text_direction(chars: &[&RawChar]) -> (TextDirection, ValueOrigin) {
    let vertical_chars = chars
        .iter()
        .filter(|ch| is_vertical_rotation(ch.rotation_degrees))
        .count();
    if vertical_chars * 2 >= chars.len().max(1) {
        return (TextDirection::TopToBottom, ValueOrigin::Observed);
    }

    let mut rtl_chars = 0;
    let mut ltr_chars = 0;
    for ch in chars {
        if is_rtl_text(ch.text as u32) {
            rtl_chars += 1;
        } else if ch.text.is_alphabetic() {
            ltr_chars += 1;
        }
    }

    let direction = match (rtl_chars > 0, ltr_chars > 0) {
        (true, true) => TextDirection::Mixed,
        (true, false) => TextDirection::RightToLeft,
        _ => TextDirection::LeftToRight,
    };
    let origin = if rtl_chars > 0 || ltr_chars > 0 {
        ValueOrigin::Reconstructed
    } else {
        ValueOrigin::Estimated
    };
    (direction, origin)
}

pub(super) fn ordered_chars_for_span<'a>(
    chars: &[&'a RawChar],
    direction: TextDirection,
) -> Vec<&'a RawChar> {
    let mut ordered = chars.to_vec();
    match direction {
        TextDirection::RightToLeft => ordered.sort_by(|a, b| {
            b.raw_x
                .partial_cmp(&a.raw_x)
                .unwrap_or(Ordering::Equal)
                .then_with(|| {
                    b.origin_x
                        .partial_cmp(&a.origin_x)
                        .unwrap_or(Ordering::Equal)
                })
        }),
        TextDirection::TopToBottom => ordered.sort_by(|a, b| {
            b.raw_y
                .partial_cmp(&a.raw_y)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.raw_x.partial_cmp(&b.raw_x).unwrap_or(Ordering::Equal))
        }),
        _ => {}
    }
    ordered
}

pub(super) fn inter_char_gap(prev: &RawChar, curr: &RawChar, direction: TextDirection) -> f32 {
    match direction {
        TextDirection::RightToLeft => prev.raw_x - (curr.raw_x + curr.raw_width),
        TextDirection::TopToBottom => prev.raw_y - (curr.raw_y + curr.raw_height),
        _ => {
            if prev.advance_width > 0.0 {
                curr.origin_x - (prev.origin_x + prev.advance_width)
            } else {
                curr.raw_x - (prev.raw_x + prev.raw_width)
            }
        }
    }
}

pub(super) fn synthesize_space_bbox(
    prev: &RawChar,
    curr: &RawChar,
    gap: f32,
    direction: TextDirection,
    page_dims: &PageDimensions,
) -> BoundingBox {
    match direction {
        TextDirection::RightToLeft => page_dims.normalize_bbox(
            curr.raw_x + curr.raw_width,
            curr.raw_y,
            gap.max(0.0),
            prev.raw_height.max(curr.raw_height),
            true,
        ),
        TextDirection::TopToBottom => page_dims.normalize_bbox(
            prev.raw_x.min(curr.raw_x),
            curr.raw_y + curr.raw_height,
            prev.raw_width.max(curr.raw_width),
            gap.max(0.0),
            true,
        ),
        _ => page_dims.normalize_bbox(
            prev.raw_x + prev.raw_width,
            prev.raw_y,
            gap.max(0.0),
            prev.raw_height,
            true,
        ),
    }
}
