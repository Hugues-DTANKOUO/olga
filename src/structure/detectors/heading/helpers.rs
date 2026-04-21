use crate::model::{Primitive, PrimitiveKind};

/// Build a mapping from font size to heading level.
///
/// Collects all distinct font sizes on the page that are significantly
/// larger than body text, sorts descending, and assigns levels 1..=6.
pub(super) fn build_level_map(
    page: &[Primitive],
    body_size: f32,
    min_ratio: f32,
) -> Vec<(f32, u8)> {
    let threshold = body_size * min_ratio;

    let mut sizes: Vec<f32> = page
        .iter()
        .filter_map(|p| match &p.kind {
            PrimitiveKind::Text { font_size, .. } if *font_size >= threshold => Some(*font_size),
            _ => None,
        })
        .collect();

    sizes.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    sizes.dedup_by(|a, b| (*a - *b).abs() < 0.5);

    sizes
        .into_iter()
        .enumerate()
        .map(|(i, size)| (size, (i as u8 + 1).min(6)))
        .collect()
}

/// Look up the heading level for a given font size from the level map.
///
/// Uses a tolerance of 0.5pt for matching.
pub(super) fn level_for_size(level_map: &[(f32, u8)], font_size: f32) -> u8 {
    for &(size, level) in level_map {
        if (font_size - size).abs() < 0.5 {
            return level;
        }
    }
    1
}
