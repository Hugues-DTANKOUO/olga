use pdf_oxide::PdfDocument;
use pdf_oxide::structure::MarkInfo;

use crate::error::{Warning, WarningKind};

use super::traversal::build_tagged_pdf_context;
use super::{ImageAltBinding, TaggedImageAppearance, TaggedPageContext, TaggedPdfContext};

pub(crate) fn load_tagged_pdf_context(
    doc: &mut PdfDocument,
    warnings: &mut Vec<Warning>,
) -> TaggedPdfContext {
    let mark_info = match doc.mark_info() {
        Ok(mark_info) => mark_info,
        Err(err) => {
            warnings.push(Warning {
                kind: WarningKind::PartialExtraction,
                message: format!("Failed to read PDF MarkInfo metadata: {}", err),
                page: None,
            });
            MarkInfo::default()
        }
    };

    let struct_tree = match doc.structure_tree() {
        Ok(struct_tree) => struct_tree,
        Err(err) => {
            warnings.push(Warning {
                kind: WarningKind::MalformedContent,
                message: format!("Failed to parse tagged PDF structure tree: {}", err),
                page: None,
            });
            return TaggedPdfContext::default();
        }
    };

    let Some(struct_tree) = struct_tree else {
        if mark_info.marked {
            warnings.push(Warning {
                kind: WarningKind::PartialExtraction,
                message:
                    "PDF declares /MarkInfo.Marked but does not expose a readable structure tree"
                        .to_string(),
                page: None,
            });
        }
        return TaggedPdfContext::default();
    };

    if mark_info.suspects {
        warnings.push(Warning {
            kind: WarningKind::PartialExtraction,
            message:
                "Tagged PDF structure tree is marked suspect; falling back to physical text order"
                    .to_string(),
            page: None,
        });
        return TaggedPdfContext::default();
    }

    if !mark_info.marked {
        warnings.push(Warning {
            kind: WarningKind::PartialExtraction,
            message:
                "PDF exposes a structure tree but /MarkInfo.Marked is false; using structure-derived order and hints"
                    .to_string(),
            page: None,
        });
    }

    let (context, summary) = build_tagged_pdf_context(&struct_tree, true);
    let (semantic_node_count, object_ref_count) = context.semantic_graph_stats();
    debug_assert_eq!(semantic_node_count, context.semantic_node_count_for_debug());
    debug_assert_eq!(object_ref_count as u32, summary.object_ref_count);

    if summary.alt_text_without_mcid > 0 {
        warnings.push(Warning {
            kind: WarningKind::PartialExtraction,
            message: format!(
                "Tagged PDF contains {} figure/formula/form alt-text entries without descendant MCID bindings on the same page",
                summary.alt_text_without_mcid
            ),
            page: None,
        });
    }
    if summary.expansion_without_mcid > 0 {
        warnings.push(Warning {
            kind: WarningKind::PartialExtraction,
            message: format!(
                "Tagged PDF contains {} /E expansion entries without descendant MCID bindings on the same page",
                summary.expansion_without_mcid
            ),
            page: None,
        });
    }
    if summary.actual_text_without_mcid > 0 {
        warnings.push(Warning {
            kind: WarningKind::PartialExtraction,
            message: format!(
                "Tagged PDF contains {} /ActualText entries without descendant MCID bindings on the same page",
                summary.actual_text_without_mcid
            ),
            page: None,
        });
    }
    if summary.object_ref_count > 0 {
        warnings.push(Warning {
            kind: WarningKind::PartialExtraction,
            message: format!(
                "Tagged PDF contains {} structure-tree ObjectRef entries; captured in the semantic graph but not yet anchored to emitted primitives",
                summary.object_ref_count
            ),
            page: None,
        });
    }

    context
}

pub(crate) fn bind_alt_text_to_image_appearances(
    page_ctx: Option<&TaggedPageContext>,
    appearances: Option<&[TaggedImageAppearance]>,
    image_count: usize,
) -> ImageAltBinding {
    let Some(page_ctx) = page_ctx else {
        return ImageAltBinding::Unavailable;
    };

    if !page_ctx.has_alt_text_candidates() {
        return ImageAltBinding::Unavailable;
    }

    let Some(appearances) = appearances else {
        return ImageAltBinding::Unavailable;
    };

    if appearances.len() != image_count {
        return ImageAltBinding::CountMismatch {
            appearance_count: appearances.len(),
            image_count,
        };
    }

    ImageAltBinding::Resolved(
        appearances
            .iter()
            .map(|appearance| page_ctx.resolve_alt_text_for_mcid(appearance.mcid))
            .collect(),
    )
}
