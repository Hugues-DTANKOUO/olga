use crate::model::{HintKind, Primitive, PrimitiveKind, SemanticHint};
use crate::structure::detectors::{DetectedHint, DetectionContext, StructureDetector};

use super::config::HeadingDetector;
use super::helpers::{build_level_map, level_for_size};

impl StructureDetector for HeadingDetector {
    fn name(&self) -> &str {
        "heading"
    }

    fn detect(&self, page: &[Primitive], context: &DetectionContext) -> Vec<DetectedHint> {
        let body_size = match context.font_histogram.body_font_size() {
            Some(s) => s,
            None => return Vec::new(),
        };

        let level_map = build_level_map(page, body_size, self.min_size_ratio);
        let mut hints = Vec::new();

        for (i, prim) in page.iter().enumerate() {
            if !prim.hints.is_empty() {
                continue;
            }

            let (font_size, is_bold, content) = match &prim.kind {
                PrimitiveKind::Text {
                    font_size,
                    is_bold,
                    content,
                    ..
                } => (*font_size, *is_bold, content.as_str()),
                _ => continue,
            };

            let char_count = content.chars().count();
            if char_count < self.min_text_len || char_count > self.max_text_len {
                continue;
            }

            let mut alpha_count: usize = 0;
            let mut upper_count: usize = 0;
            let mut contains_colon = false;
            for c in content.chars() {
                if c.is_alphabetic() {
                    alpha_count += 1;
                    if c.is_uppercase() {
                        upper_count += 1;
                    }
                } else if c == ':' {
                    contains_colon = true;
                }
            }

            if char_count > 0 && (alpha_count as f32 / char_count as f32) < 0.4 {
                continue;
            }

            let in_top_percentile = context
                .font_histogram
                .is_in_top_percentile(font_size, self.percentile);
            let is_large = in_top_percentile && font_size >= body_size * self.min_size_ratio;

            let is_allcaps = is_bold
                && (3..=60).contains(&char_count)
                && !contains_colon
                && alpha_count > 0
                && upper_count == alpha_count;

            let word_count = content.split_whitespace().count();
            let is_bold_short_title = is_bold
                && !is_large
                && !is_allcaps
                && (3..=50).contains(&char_count)
                && word_count <= 6
                && !content.ends_with('.')
                && !content.ends_with(',')
                && !contains_colon
                && !content.contains(',')
                && content.chars().next().is_some_and(|c| c.is_uppercase());

            if !is_large && !is_allcaps && !is_bold_short_title {
                continue;
            }

            let level = if is_large {
                level_for_size(&level_map, font_size)
            } else {
                2
            };

            let confidence = if is_large && is_bold {
                self.bold_confidence
            } else if is_large {
                self.plain_confidence
            } else if is_allcaps {
                0.72
            } else {
                0.71
            };

            hints.push(DetectedHint {
                primitive_index: i,
                hint: SemanticHint::from_heuristic(
                    HintKind::Heading { level },
                    confidence,
                    "HeadingDetector",
                ),
            });
        }

        hints
    }
}
