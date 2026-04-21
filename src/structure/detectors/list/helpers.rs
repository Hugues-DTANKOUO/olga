use crate::model::{Primitive, PrimitiveKind};

/// Find the dominant (most common) left X coordinate from textboxes.
pub(super) fn dominant_box_left_margin(boxes: &[crate::structure::layout::TextBox]) -> f32 {
    if boxes.is_empty() {
        return 0.0;
    }

    let mut bins: std::collections::HashMap<i32, usize> = std::collections::HashMap::new();
    for tb in boxes {
        let bin = (tb.bbox.x * 100.0).round() as i32;
        *bins.entry(bin).or_insert(0) += 1;
    }

    let best_bin = bins
        .into_iter()
        .max_by_key(|&(_, count)| count)
        .map(|(bin, _)| bin)
        .unwrap_or(0);

    best_bin as f32 / 100.0
}

/// Check if there's a non-empty text primitive between two indices.
///
/// Returns `true` if any text primitive in `(start..end)` contains
/// non-whitespace text (indicating a paragraph break between list items).
pub(super) fn has_text_gap(page: &[Primitive], start: usize, end: usize) -> bool {
    for i in (start + 1)..end {
        if i >= page.len() {
            break;
        }
        if let PrimitiveKind::Text { ref content, .. } = page[i].kind
            && !content.trim().is_empty()
        {
            return true;
        }
    }
    false
}

/// Find the dominant (most common) left X coordinate of text items.
pub(super) fn dominant_left_margin(items: &[(usize, &Primitive, &str)]) -> f32 {
    if items.is_empty() {
        return 0.0;
    }

    // Bucket X coordinates into bins of 0.01 width.
    let mut bins: std::collections::HashMap<i32, usize> = std::collections::HashMap::new();
    for &(_, prim, _) in items {
        let bin = (prim.bbox.x * 100.0).round() as i32;
        *bins.entry(bin).or_insert(0) += 1;
    }

    // Most frequent bin.
    let best_bin = bins
        .into_iter()
        .max_by_key(|&(_, count)| count)
        .map(|(bin, _)| bin)
        .unwrap_or(0);

    best_bin as f32 / 100.0
}
