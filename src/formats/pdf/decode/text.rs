use pdf_oxide::PdfDocument;
use pdf_oxide::layout::FontWeight;
use pdf_oxide::layout::TextSpan as LayoutTextSpan;

use crate::error::{Warning, WarningKind};
use crate::model::*;
use crate::pipeline::PrimitiveSink;

use super::super::artifact_text::{TextArtifactRule, suppress_artifact_text_spans};
use super::super::language::harmonize_text_direction_with_language;
use super::super::pages::{
    clean_font_name, group_chars_into_spans, is_bold_font, is_italic_font, is_non_bold_font,
    layout_spans_to_text_spans, reorder_spans_by_lines,
};
use super::super::tagged::{
    TaggedPdfContext, TaggedTextApplicationResult, TaggedTextSpan,
    apply_tagged_structure_to_text_spans,
};
use super::super::types::{RawChar, TextSpan};

pub(super) struct PageTextDecodeContext<'a> {
    pub doc: &'a mut PdfDocument,
    pub page_idx: usize,
    pub page_num: u32,
    pub page_dims: &'a PageDimensions,
    pub declared_document_lang: Option<&'a String>,
    pub tagged_context: &'a TaggedPdfContext,
    pub artifact_rules: Option<&'a [TextArtifactRule]>,
    pub cached_page0_spans: &'a mut Option<Vec<LayoutTextSpan>>,
    pub source_order: &'a mut u64,
    pub warnings: &'a mut Vec<Warning>,
    pub primitives: &'a mut Vec<Primitive>,
    pub extracted_document_chars: &'a mut Vec<char>,
}

pub(super) fn extract_page_text(ctx: PageTextDecodeContext<'_>) {
    let PageTextDecodeContext {
        doc,
        page_idx,
        page_num,
        page_dims,
        declared_document_lang,
        tagged_context,
        artifact_rules,
        cached_page0_spans,
        source_order,
        warnings,
        primitives,
        extracted_document_chars,
    } = ctx;

    let use_spans = page_idx == 0;
    let layout_spans_opt: Option<Vec<TextSpan>> = if use_spans {
        let raw_spans = if page_idx == 0 {
            cached_page0_spans
                .take()
                .or_else(|| doc.extract_spans(page_idx).ok())
        } else {
            doc.extract_spans(page_idx).ok()
        };
        match raw_spans {
            Some(ls) if !ls.is_empty() => {
                let s = layout_spans_to_text_spans(ls, page_dims);
                if s.is_empty() {
                    None
                } else {
                    // Reorder by visual line clustering so skewed scans (where
                    // pdf_oxide can emit spans right-to-left along a tilted
                    // baseline) read top-to-bottom, left-to-right.
                    Some(reorder_spans_by_lines(s))
                }
            }
            _ => None,
        }
    } else {
        None
    };

    if let Some(spans) = layout_spans_opt {
        extracted_document_chars.extend(spans.iter().flat_map(|s| s.content.chars()));
        let tagged_result =
            build_tagged_text_result(spans, page_num, tagged_context, artifact_rules, warnings);
        emit_tagged_text_primitives(
            tagged_result,
            page_num,
            declared_document_lang,
            source_order,
            warnings,
            primitives,
        );
        return;
    }

    match doc.extract_chars(page_idx) {
        Ok(chars) => {
            let raw_chars: Vec<RawChar> = chars
                .iter()
                .filter_map(|ch| {
                    let c = ch.char;
                    if c.is_control() && c != '\n' && c != '\t' {
                        return None;
                    }

                    let font_name = clean_font_name(&ch.font_name);
                    let bold_from_name = is_bold_font(&ch.font_name);
                    let bold_from_weight = matches!(
                        ch.font_weight,
                        FontWeight::Bold | FontWeight::ExtraBold | FontWeight::Black
                    );
                    let name_says_not_bold = is_non_bold_font(&ch.font_name);
                    let is_bold = if name_says_not_bold {
                        false
                    } else {
                        bold_from_name || bold_from_weight
                    };
                    let is_italic = is_italic_font(&ch.font_name) || ch.is_italic;

                    Some(RawChar {
                        text: c,
                        raw_x: ch.bbox.x,
                        raw_y: ch.bbox.y,
                        raw_width: ch.bbox.width,
                        raw_height: ch.bbox.height,
                        font_name,
                        font_size: ch.font_size,
                        is_bold,
                        is_italic,
                        color_r: ch.color.r,
                        color_g: ch.color.g,
                        color_b: ch.color.b,
                        is_monospace: ch.is_monospace,
                        origin_x: ch.origin_x,
                        origin_y: ch.origin_y,
                        advance_width: ch.advance_width,
                        rotation_degrees: ch.rotation_degrees,
                        mcid: ch.mcid,
                    })
                })
                .collect();

            extracted_document_chars.extend(raw_chars.iter().map(|raw_char| raw_char.text));

            let spans = group_chars_into_spans(&raw_chars, page_dims);
            let tagged_result =
                build_tagged_text_result(spans, page_num, tagged_context, artifact_rules, warnings);

            emit_tagged_text_primitives(
                tagged_result,
                page_num,
                declared_document_lang,
                source_order,
                warnings,
                primitives,
            );
        }
        Err(e) => {
            warnings.push(Warning {
                kind: WarningKind::MalformedContent,
                message: format!("Failed to extract text from page {}: {}", page_idx, e),
                page: Some(page_num),
            });
        }
    }
}

fn build_tagged_text_result(
    spans: Vec<TextSpan>,
    page_num: u32,
    tagged_context: &TaggedPdfContext,
    artifact_rules: Option<&[TextArtifactRule]>,
    warnings: &mut Vec<Warning>,
) -> TaggedTextApplicationResult {
    let (spans, artifact_stats) = suppress_artifact_text_spans(spans, artifact_rules);
    if artifact_stats.total() > 0 {
        warnings.push(Warning {
            kind: WarningKind::FilteredArtifact,
            message: artifact_stats.describe(),
            page: Some(page_num),
        });
    }

    if tagged_context.use_structure_reading_order() {
        if let Some(page_ctx) = tagged_context.page_context(page_num) {
            apply_tagged_structure_to_text_spans(spans, page_ctx)
        } else {
            TaggedTextApplicationResult {
                spans: spans.into_iter().map(TaggedTextSpan::plain).collect(),
                stats: Default::default(),
            }
        }
    } else {
        TaggedTextApplicationResult {
            spans: spans.into_iter().map(TaggedTextSpan::plain).collect(),
            stats: Default::default(),
        }
    }
}

fn emit_tagged_text_primitives(
    tagged_result: TaggedTextApplicationResult,
    page_num: u32,
    declared_document_lang: Option<&String>,
    source_order: &mut u64,
    warnings: &mut Vec<Warning>,
    primitives: &mut Vec<Primitive>,
) {
    if tagged_result.stats.unmatched_structure_item_count > 0 {
        warnings.push(Warning {
            kind: WarningKind::PartialExtraction,
            message: format!(
                "Tagged PDF structure tree on page {} referenced {} text item(s) that could not be matched to extracted spans",
                page_num, tagged_result.stats.unmatched_structure_item_count
            ),
            page: Some(page_num),
        });
    }

    if tagged_result.stats.fallback_span_count > 0 {
        warnings.push(Warning {
            kind: WarningKind::PartialExtraction,
            message: format!(
                "Tagged PDF page {} emitted {} text span(s) in physical fallback order because they were not bound into the structure tree",
                page_num, tagged_result.stats.fallback_span_count
            ),
            page: Some(page_num),
        });
    }

    if tagged_result.stats.fallback_span_count > 0
        || tagged_result.stats.unmatched_structure_item_count > 0
    {
        let total_spans = tagged_result.spans.len();
        let structured_spans = total_spans.saturating_sub(tagged_result.stats.fallback_span_count);
        warnings.push(Warning {
            kind: WarningKind::PartialExtraction,
            message: format!(
                "Tagged PDF page {} structure coverage: {} of {} emitted text span(s) were bound to the structure tree, with {} fallback span(s) and {} unmatched structure item(s)",
                page_num,
                structured_spans,
                total_spans,
                tagged_result.stats.fallback_span_count,
                tagged_result.stats.unmatched_structure_item_count
            ),
            page: Some(page_num),
        });
    }

    if tagged_result.stats.unapplied_structural_hint_count > 0 {
        warnings.push(Warning {
            kind: WarningKind::PartialExtraction,
            message: format!(
                "Tagged PDF page {} emitted {} structural boundary hint(s) that could not be anchored to text spans on this page",
                page_num, tagged_result.stats.unapplied_structural_hint_count
            ),
            page: Some(page_num),
        });
    }

    for tagged_span in tagged_result.spans {
        let span = tagged_span.span;
        let char_positions: Vec<CharPosition> = span
            .char_bboxes
            .iter()
            .map(|(c, bbox)| CharPosition {
                ch: *c,
                bbox: *bbox,
            })
            .collect();

        let effective_lang = tagged_span
            .lang
            .clone()
            .or_else(|| declared_document_lang.cloned());
        let (text_direction, text_direction_origin) = harmonize_text_direction_with_language(
            span.text_direction,
            span.text_direction_origin,
            effective_lang.as_deref(),
        );
        let mut prim = Primitive::new(
            PrimitiveKind::Text {
                content: span.content,
                font_size: span.font_size,
                font_name: span.font_name,
                is_bold: span.is_bold,
                is_italic: span.is_italic,
                color: span.color,
                char_positions: if char_positions.is_empty() {
                    None
                } else {
                    Some(char_positions)
                },
                text_direction,
                text_direction_origin,
                lang: effective_lang,
                lang_origin: tagged_span
                    .lang_origin
                    .or_else(|| declared_document_lang.map(|_| ValueOrigin::Reconstructed)),
            },
            span.bbox,
            page_num,
            *source_order,
            tagged_span.geometry_provenance,
        );
        if !tagged_span.hints.is_empty() {
            prim = prim.with_hints(tagged_span.hints);
        }
        *source_order += 1;
        primitives.emit(prim);
    }
}
