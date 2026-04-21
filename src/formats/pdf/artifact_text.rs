use std::collections::{HashMap, HashSet};

use pdf_oxide::PdfDocument;
use pdf_oxide::extractors::text::ArtifactType;
use pdf_oxide::layout::TextSpan as LayoutTextSpan;
use unicode_normalization::UnicodeNormalization;

use crate::error::{Warning, WarningKind};
use crate::model::BoundingBox;

use super::metadata;
use super::tagged::TaggedPageContext;
use super::types::TextSpan;

const PDF_ARTIFACT_TOP_BAND: f32 = 0.15;
const PDF_ARTIFACT_BOTTOM_BAND: f32 = 0.85;
const MIN_REPEATED_ARTIFACT_OCCURRENCES: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum ArtifactBand {
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ArtifactRuleSource {
    ExplicitTaggedPagination,
    RepeatedTaggedResidual,
}

#[derive(Debug, Clone)]
pub(super) struct TextArtifactRule {
    pub(super) mcid: Option<u32>,
    pub(super) text: String,
    pub(super) band: ArtifactBand,
    pub(super) source: ArtifactRuleSource,
}

#[derive(Debug, Clone)]
pub(super) struct HeuristicArtifactCandidate {
    pub(super) page: u32,
    pub(super) mcid: Option<u32>,
    pub(super) text: String,
    pub(super) band: ArtifactBand,
}

#[derive(Debug, Default, Clone)]
pub(super) struct TextArtifactSuppressionPlan {
    rules_by_page: HashMap<u32, Vec<TextArtifactRule>>,
}

impl TextArtifactSuppressionPlan {
    pub(super) fn add_rule(&mut self, page: u32, rule: TextArtifactRule) {
        self.rules_by_page.entry(page).or_default().push(rule);
    }

    pub(super) fn rules_for_page(&self, page: u32) -> Option<&[TextArtifactRule]> {
        self.rules_by_page.get(&page).map(Vec::as_slice)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct ArtifactSuppressionStats {
    pub(super) explicit_tagged_pagination: u32,
    pub(super) repeated_tagged_residual: u32,
}

impl ArtifactSuppressionStats {
    fn record(&mut self, source: ArtifactRuleSource) {
        match source {
            ArtifactRuleSource::ExplicitTaggedPagination => self.explicit_tagged_pagination += 1,
            ArtifactRuleSource::RepeatedTaggedResidual => self.repeated_tagged_residual += 1,
        }
    }

    pub(super) fn total(&self) -> u32 {
        self.explicit_tagged_pagination + self.repeated_tagged_residual
    }

    pub(super) fn describe(&self) -> String {
        match (
            self.explicit_tagged_pagination,
            self.repeated_tagged_residual,
        ) {
            (explicit, 0) => format!(
                "Suppressed {} tagged PDF pagination artifact span(s) from text output",
                explicit
            ),
            (0, repeated) => format!(
                "Suppressed {} repeated top/bottom residual span(s) from tagged PDF text output",
                repeated
            ),
            (explicit, repeated) => format!(
                "Suppressed {} tagged pagination artifact span(s) and {} repeated top/bottom residual span(s) from tagged PDF text output",
                explicit, repeated
            ),
        }
    }
}

/// Build the artifact suppression plan and optionally cache page-0 layout spans.
///
/// Returns `(plan, cached_page0_spans)`. The cached spans can be reused in the
/// main decode loop to avoid a redundant `extract_spans(0)` call.
pub(super) fn build_text_artifact_suppression_plan(
    doc: &mut PdfDocument,
    tagged_context: &super::tagged::TaggedPdfContext,
    warnings: &mut Vec<Warning>,
) -> (TextArtifactSuppressionPlan, Option<Vec<LayoutTextSpan>>) {
    let mut plan = TextArtifactSuppressionPlan::default();

    let page_count = match doc.page_count() {
        Ok(page_count) => page_count,
        Err(err) => {
            warnings.push(Warning {
                kind: WarningKind::PartialExtraction,
                message: format!(
                    "Failed to count PDF pages during artifact qualification: {}",
                    err
                ),
                page: None,
            });
            return (plan, None);
        }
    };

    let mut heuristic_candidates = Vec::new();
    let mut cached_page0_spans: Option<Vec<LayoutTextSpan>> = None;

    for page_idx in 0..page_count {
        let page_num = page_idx as u32;
        let page_dims = metadata::extract_page_dims(doc, page_idx, warnings);
        let spans = match doc.extract_spans(page_idx) {
            Ok(spans) => spans,
            Err(err) => {
                warnings.push(Warning {
                    kind: WarningKind::PartialExtraction,
                    message: format!(
                        "Failed to extract PDF spans during artifact qualification on page {}: {}",
                        page_num, err
                    ),
                    page: Some(page_num),
                });
                continue;
            }
        };

        let page_ctx = tagged_context.page_context(page_num);
        let page_has_structure = tagged_context.use_structure_reading_order()
            && page_ctx.is_some_and(TaggedPageContext::has_reading_items);

        for span in &spans {
            let Some(text) = normalize_artifact_text(&span.text) else {
                continue;
            };
            let bbox = page_dims.normalize_bbox(
                span.bbox.x,
                span.bbox.y,
                span.bbox.width,
                span.bbox.height,
                true,
            );
            let Some(band) = classify_artifact_band(bbox) else {
                continue;
            };

            if matches!(span.artifact_type, Some(ArtifactType::Pagination(_))) {
                plan.add_rule(
                    page_num,
                    TextArtifactRule {
                        mcid: span.mcid,
                        text,
                        band,
                        source: ArtifactRuleSource::ExplicitTaggedPagination,
                    },
                );
                continue;
            }

            if page_has_structure
                && page_ctx.is_some_and(|ctx| is_unstructured_tagged_span_candidate(span.mcid, ctx))
                && should_keep_repeated_artifact_candidate(&text)
            {
                heuristic_candidates.push(HeuristicArtifactCandidate {
                    page: page_num,
                    mcid: span.mcid,
                    text,
                    band,
                });
            }
        }

        // Cache page 0 spans (zero-copy move) for reuse in the main decode loop.
        if page_idx == 0 {
            cached_page0_spans = Some(spans);
        }
    }

    add_repeated_tagged_residual_rules(&mut plan, &heuristic_candidates);
    (plan, cached_page0_spans)
}

pub(super) fn add_repeated_tagged_residual_rules(
    plan: &mut TextArtifactSuppressionPlan,
    candidates: &[HeuristicArtifactCandidate],
) {
    let mut occurrences: HashMap<(ArtifactBand, String), HashSet<u32>> = HashMap::new();
    for candidate in candidates {
        occurrences
            .entry((candidate.band, candidate.text.clone()))
            .or_default()
            .insert(candidate.page);
    }

    for candidate in candidates {
        let key = (candidate.band, candidate.text.clone());
        if occurrences
            .get(&key)
            .is_some_and(|pages| pages.len() >= MIN_REPEATED_ARTIFACT_OCCURRENCES)
        {
            plan.add_rule(
                candidate.page,
                TextArtifactRule {
                    mcid: candidate.mcid,
                    text: candidate.text.clone(),
                    band: candidate.band,
                    source: ArtifactRuleSource::RepeatedTaggedResidual,
                },
            );
        }
    }
}

pub(super) fn suppress_artifact_text_spans(
    spans: Vec<TextSpan>,
    rules: Option<&[TextArtifactRule]>,
) -> (Vec<TextSpan>, ArtifactSuppressionStats) {
    let Some(rules) = rules else {
        return (spans, ArtifactSuppressionStats::default());
    };

    let mut kept = Vec::with_capacity(spans.len());
    let mut stats = ArtifactSuppressionStats::default();

    for span in spans {
        if let Some(rule) = rules.iter().find(|rule| artifact_rule_matches(&span, rule)) {
            stats.record(rule.source);
        } else {
            kept.push(span);
        }
    }

    (kept, stats)
}

pub(super) fn classify_artifact_band(bbox: BoundingBox) -> Option<ArtifactBand> {
    if bbox.y <= PDF_ARTIFACT_TOP_BAND {
        Some(ArtifactBand::Top)
    } else if bbox.bottom() >= PDF_ARTIFACT_BOTTOM_BAND {
        Some(ArtifactBand::Bottom)
    } else {
        None
    }
}

fn artifact_rule_matches(span: &TextSpan, rule: &TextArtifactRule) -> bool {
    if let Some(rule_mcid) = rule.mcid
        && span.mcid == Some(rule_mcid)
    {
        return true;
    }

    let Some(span_text) = normalize_artifact_text(&span.content) else {
        return false;
    };
    span_text == rule.text && classify_artifact_band(span.bbox) == Some(rule.band)
}

fn is_unstructured_tagged_span_candidate(mcid: Option<u32>, page_ctx: &TaggedPageContext) -> bool {
    match mcid {
        Some(mcid) => !page_ctx.references_mcid(mcid),
        None => true,
    }
}

fn should_keep_repeated_artifact_candidate(text: &str) -> bool {
    text.chars().count() > 3 && !text.chars().all(|ch| ch.is_numeric())
}

fn normalize_artifact_text(text: &str) -> Option<String> {
    let normalized = text.nfc().collect::<String>();
    let collapsed = normalized.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}
