//! PDF annotation-based link extraction.
//!
//! LaTeX/hyperref and many other PDF producers create hyperlinks as page-level
//! annotations (`/Annot /Subtype /Link /A << /S /URI /URI (url) >>`) rather
//! than Tagged PDF structure tree links. This module extracts those annotations
//! and attaches `HintKind::Link` hints to text primitives whose bounding boxes
//! overlap the annotation rectangle.

mod extract;
mod overlap;
#[cfg(test)]
mod tests;

use pdf_oxide::PdfDocument;

use crate::error::{Warning, WarningKind};
use crate::model::{HintKind, HintSource, PageDimensions, Primitive, PrimitiveKind, SemanticHint};

use extract::extract_link_annotations_inner;
use overlap::overlap_score;

/// Extract link annotations from a single PDF page and attach them as
/// `HintKind::Link` hints to matching text primitives.
///
/// Annotations are matched to primitives by bounding-box overlap: a primitive
/// is considered covered if at least 50% of its width overlaps the annotation
/// rectangle horizontally AND their vertical extents overlap.
///
/// When `cached_page_dict` is provided, the page tree traversal is skipped
/// entirely — the caller has already resolved the page dictionary in a
/// single-pass cache.
pub(crate) fn apply_link_annotations(
    doc: &mut PdfDocument,
    page_idx: usize,
    page_dims: &PageDimensions,
    primitives: &mut [Primitive],
    warnings: &mut Vec<Warning>,
    cached_page_dict: Option<&std::collections::HashMap<String, pdf_oxide::object::Object>>,
) {
    let page_num = page_idx as u32;

    let annotations =
        match extract_link_annotations_inner(doc, page_idx, page_dims, cached_page_dict) {
            Ok(annots) => annots,
            Err(msg) => {
                if !msg.contains("No /Annots") && !msg.contains("not found in") {
                    warnings.push(Warning {
                        kind: WarningKind::PartialExtraction,
                        message: format!(
                            "Page {}: failed to extract link annotations: {}",
                            page_idx, msg
                        ),
                        page: Some(page_num),
                    });
                }
                return;
            }
        };

    if annotations.is_empty() {
        return;
    }

    for prim in primitives.iter_mut() {
        if prim.page != page_num {
            continue;
        }
        if !matches!(prim.kind, PrimitiveKind::Text { .. }) {
            continue;
        }
        if prim
            .hints
            .iter()
            .any(|h| matches!(h.kind, HintKind::Link { .. }))
        {
            continue;
        }

        let best = annotations
            .iter()
            .filter_map(|annot| overlap_score(&prim.bbox, &annot.bbox).map(|score| (annot, score)))
            .max_by(|a, b| a.1.total_cmp(&b.1));

        if let Some((annot, _score)) = best {
            prim.hints.push(SemanticHint {
                kind: HintKind::Link {
                    url: annot.url.clone(),
                },
                confidence: 0.95,
                source: HintSource::FormatDerived,
            });
        }
    }
}
