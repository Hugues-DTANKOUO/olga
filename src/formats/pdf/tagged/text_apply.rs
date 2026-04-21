use pdf_oxide::text::rtl_detector::is_rtl_text;

use super::super::types::TextSpan;
use super::*;
use crate::model::{GeometryProvenance, GeometrySpace, SemanticHint, TextDirection, ValueOrigin};

pub(crate) fn apply_tagged_structure_to_text_spans(
    spans: Vec<TextSpan>,
    page_ctx: &TaggedPageContext,
) -> TaggedTextApplicationResult {
    if spans.is_empty() {
        return TaggedTextApplicationResult {
            spans: Vec::new(),
            stats: TaggedTextApplicationStats::default(),
        };
    }

    if page_ctx.reading_items.is_empty() {
        let tagged_spans = spans
            .into_iter()
            .map(|span| {
                let metadata = page_ctx.metadata_for_mcid(span.mcid);
                TaggedTextSpan::with_metadata(
                    span,
                    metadata,
                    GeometryProvenance::observed_physical(),
                )
            })
            .collect();
        return TaggedTextApplicationResult {
            spans: tagged_spans,
            stats: TaggedTextApplicationStats::default(),
        };
    }

    let mut emitted: Vec<TaggedTextSpan> = Vec::with_capacity(spans.len());
    let mut used = vec![false; spans.len()];
    let mut stats = TaggedTextApplicationStats::default();
    let mut pending_structural_hints = Vec::new();

    for item in &page_ctx.reading_items {
        match item {
            TaggedReadingItem::StructuralStart { hint } => {
                pending_structural_hints.push(hint.clone());
            }
            TaggedReadingItem::StructuralEnd { hint } => {
                if let Some(last_span) = emitted.last_mut() {
                    add_hint_if_missing(&mut last_span.hints, hint.clone());
                } else {
                    stats.unapplied_structural_hint_count += 1;
                }
            }
            TaggedReadingItem::Mcid { mcid, metadata } => {
                let mut matched = false;
                for (idx, span) in spans.iter().enumerate() {
                    if !used[idx] && span.mcid == Some(*mcid) {
                        let mut tagged_span = TaggedTextSpan::with_metadata(
                            span.clone(),
                            metadata.clone(),
                            GeometryProvenance::observed_physical(),
                        );
                        attach_pending_structural_hints(
                            &mut tagged_span.hints,
                            &mut pending_structural_hints,
                        );
                        emitted.push(tagged_span);
                        used[idx] = true;
                        matched = true;
                    }
                }
                if !matched {
                    stats.unmatched_structure_item_count += 1;
                }
            }
            TaggedReadingItem::ActualText {
                text,
                mcids,
                metadata,
            } => {
                let descendant_indices: Vec<usize> = spans
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, span)| {
                        (!used[idx] && span.mcid.is_some_and(|mcid| mcids.contains(&mcid)))
                            .then_some(idx)
                    })
                    .collect();

                if descendant_indices.is_empty() {
                    stats.unmatched_structure_item_count += 1;
                    continue;
                }

                let descendants: Vec<TextSpan> = descendant_indices
                    .iter()
                    .map(|idx| spans[*idx].clone())
                    .collect();
                if let Some(replacement_span) = synthesize_actual_text_span(text, &descendants) {
                    let mut tagged_span = TaggedTextSpan::with_metadata(
                        replacement_span,
                        metadata.clone(),
                        GeometryProvenance::reconstructed(GeometrySpace::PhysicalPage),
                    );
                    attach_pending_structural_hints(
                        &mut tagged_span.hints,
                        &mut pending_structural_hints,
                    );
                    emitted.push(tagged_span);
                    stats.actual_text_replacement_count += 1;
                    for idx in descendant_indices {
                        used[idx] = true;
                    }
                } else {
                    stats.unmatched_structure_item_count += 1;
                }
            }
        }
    }

    for (idx, span) in spans.into_iter().enumerate() {
        if !used[idx] {
            stats.fallback_span_count += 1;
            let metadata = page_ctx.metadata_for_mcid(span.mcid);
            let mut tagged_span = TaggedTextSpan::with_metadata(
                span,
                metadata,
                GeometryProvenance::observed_physical(),
            );
            attach_pending_structural_hints(&mut tagged_span.hints, &mut pending_structural_hints);
            emitted.push(tagged_span);
        }
    }

    stats.unapplied_structural_hint_count += pending_structural_hints.len();

    TaggedTextApplicationResult {
        spans: emitted,
        stats,
    }
}

fn attach_pending_structural_hints(
    hints: &mut Vec<SemanticHint>,
    pending_structural_hints: &mut Vec<SemanticHint>,
) {
    for hint in pending_structural_hints.drain(..) {
        add_hint_if_missing(hints, hint);
    }
}

fn synthesize_actual_text_span(actual_text: &str, descendants: &[TextSpan]) -> Option<TextSpan> {
    let first = descendants.first()?;
    let content = normalize_text_value(Some(actual_text))?;
    let bbox = descendants
        .iter()
        .skip(1)
        .fold(first.bbox, |acc: crate::model::BoundingBox, span| {
            acc.merge(&span.bbox)
        });
    let text_direction = infer_replacement_text_direction(&content, first.text_direction);

    Some(TextSpan {
        content,
        bbox,
        font_name: first.font_name.clone(),
        font_size: first.font_size,
        is_bold: first.is_bold,
        is_italic: first.is_italic,
        color: first.color,
        text_direction,
        text_direction_origin: ValueOrigin::Reconstructed,
        mcid: None,
        char_bboxes: Vec::new(),
    })
}

fn infer_replacement_text_direction(replacement: &str, fallback: TextDirection) -> TextDirection {
    if replacement.chars().any(|ch| is_rtl_text(ch as u32)) {
        TextDirection::RightToLeft
    } else {
        fallback
    }
}
