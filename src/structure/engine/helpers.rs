use crate::model::{HintKind, HintSource, Primitive, SemanticHint};

use crate::structure::visual_lines::VisualLine;

pub(super) fn looks_like_multicolumn(lines: &[VisualLine]) -> bool {
    if lines.len() < 4 {
        return false;
    }

    let mut bins: std::collections::HashMap<i32, usize> = std::collections::HashMap::new();
    for line in lines {
        let bin = (line.bbox.x / 0.01).round() as i32;
        *bins.entry(bin).or_default() += 1;
    }

    let mut bin_keys: Vec<i32> = bins.keys().copied().collect();
    bin_keys.sort_unstable();

    let mut clusters: Vec<(f32, usize)> = Vec::new();
    for &bin in &bin_keys {
        let count = bins[&bin];
        let x = bin as f32 * 0.01;
        if let Some(last) = clusters.last_mut()
            && (x - last.0).abs() < 0.05
        {
            last.0 = (last.0 * last.1 as f32 + x * count as f32) / (last.1 + count) as f32;
            last.1 += count;
            continue;
        }
        clusters.push((x, count));
    }

    let min_count = (lines.len() as f32 * 0.15).ceil() as usize;
    let significant: Vec<f32> = clusters
        .iter()
        .filter(|(_, count)| *count >= min_count)
        .map(|(center, _)| *center)
        .collect();

    if significant.len() < 2 {
        return false;
    }

    for i in 0..significant.len() - 1 {
        if significant[i + 1] - significant[i] > 0.15 {
            return true;
        }
    }

    false
}

pub(super) fn tag_page_numbers(primitives: &mut [Primitive]) {
    for prim in primitives.iter_mut() {
        if let Some(text) = prim.text_content() {
            let trimmed = text.trim();
            if trimmed.is_empty() || trimmed.chars().count() > 3 {
                continue;
            }
            if !trimmed.bytes().all(|b| b.is_ascii_digit()) {
                continue;
            }
            if prim.bbox.y + prim.bbox.height < 0.88 {
                continue;
            }
            if !prim.hints.is_empty() {
                continue;
            }
            prim.hints.push(SemanticHint {
                kind: HintKind::PageFooter,
                confidence: 0.80,
                source: HintSource::HeuristicInferred {
                    detector: "page_number".to_string(),
                },
            });
        }
    }
}

pub(super) fn strip_stream_table_hints(primitives: &mut [Primitive]) {
    for prim in primitives.iter_mut() {
        prim.hints.retain(|h| {
            if h.confidence > 0.70 {
                return true;
            }
            !matches!(
                h.kind,
                HintKind::TableCell { .. }
                    | HintKind::TableCellContinuation { .. }
                    | HintKind::TableHeader { .. }
                    | HintKind::ContainedInTableCell { .. }
                    | HintKind::FlattenedFromTableCell { .. }
            )
        });
    }
}
