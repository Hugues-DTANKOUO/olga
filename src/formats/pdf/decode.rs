mod paths;
mod text;

use pdf_oxide::PdfDocument;

use crate::error::{IdpError, IdpResult, Warning};
use crate::model::*;
use crate::traits::DecodeResult;

use super::annotations::apply_link_annotations;
use super::artifact_image::{convert_images, scan_page_image_appearances};
use super::artifact_risk::ArtifactRiskCollector;
use super::artifact_text::build_text_artifact_suppression_plan;
use super::language::extract_declared_document_language;
use super::metadata;
use super::resources::build_page_cache;
use super::tagged::load_tagged_pdf_context;
use super::types::RawImage;

pub(super) fn decode_pdf(data: Vec<u8>) -> IdpResult<DecodeResult> {
    let file_size = data.len() as u64;
    let mut doc = PdfDocument::from_bytes(data)
        .map_err(|e| IdpError::UnsupportedFormat(format!("Failed to open PDF: {}", e)))?;

    let page_count = doc.page_count().map_err(|e| IdpError::Decode {
        context: "PDF decode".into(),
        message: format!("Failed to get page count: {}", e),
    })?;

    let mut primitives: Vec<Primitive> = Vec::with_capacity(page_count * 200);
    let mut warnings: Vec<Warning> = Vec::with_capacity(page_count);
    let mut images: Vec<ExtractedImage> = Vec::new();
    let mut source_order: u64 = 0;
    let mut all_page_dimensions: Vec<PageDimensions> = Vec::with_capacity(page_count);
    let declared_document_lang = extract_declared_document_language(&mut doc, &mut warnings);
    let tagged_context = load_tagged_pdf_context(&mut doc, &mut warnings);
    let (artifact_plan, mut cached_page0_spans) = if tagged_context.use_structure_reading_order() {
        build_text_artifact_suppression_plan(&mut doc, &tagged_context, &mut warnings)
    } else {
        (Default::default(), None)
    };
    let mut extracted_document_chars = Vec::new();
    let mut artifact_risk = ArtifactRiskCollector::default();

    let page_cache = build_page_cache(&mut doc, page_count).ok();

    for page_idx in 0..page_count {
        let page_prim_start = primitives.len();
        let page_dims = metadata::extract_page_dims(&mut doc, page_idx, &mut warnings);
        all_page_dimensions.push(page_dims);

        let page_num = page_idx as u32;

        text::extract_page_text(text::PageTextDecodeContext {
            doc: &mut doc,
            page_idx,
            page_num,
            page_dims: &page_dims,
            declared_document_lang: declared_document_lang.as_ref(),
            tagged_context: &tagged_context,
            artifact_rules: artifact_plan.rules_for_page(page_num),
            cached_page0_spans: &mut cached_page0_spans,
            source_order: &mut source_order,
            warnings: &mut warnings,
            primitives: &mut primitives,
            extracted_document_chars: &mut extracted_document_chars,
        });

        paths::extract_page_paths(paths::PagePathDecodeContext {
            doc: &mut doc,
            page_idx,
            page_num,
            page_dims: &page_dims,
            tagged_context: &tagged_context,
            cached_page: page_cache.as_ref().and_then(|c| c.get(page_idx)),
            artifact_risk: &mut artifact_risk,
            source_order: &mut source_order,
            warnings: &mut warnings,
            primitives: &mut primitives,
        });

        let cached_dict = page_cache
            .as_ref()
            .and_then(|c| c.get(page_idx).map(|p| &p.page_dict));
        apply_link_annotations(
            &mut doc,
            page_idx,
            &page_dims,
            &mut primitives[page_prim_start..],
            &mut warnings,
            cached_dict,
        );

        extract_page_images(
            &mut doc,
            page_idx,
            page_num,
            &page_dims,
            &tagged_context,
            &mut warnings,
            &mut images,
        );
    }

    artifact_risk.emit_warnings(&mut warnings);

    if declared_document_lang.is_none()
        && let Some(inferred_lang) =
            super::pages::infer_document_language_tag(&extracted_document_chars)
    {
        for primitive in &mut primitives {
            if let PrimitiveKind::Text {
                lang, lang_origin, ..
            } = &mut primitive.kind
                && lang.is_none()
            {
                *lang = Some(inferred_lang.clone());
                *lang_origin = Some(ValueOrigin::Estimated);
            }
        }
    }

    let metadata = DocumentMetadata {
        format: DocumentFormat::Pdf,
        page_count: page_count as u32,
        page_count_provenance: PaginationProvenance::observed(PaginationBasis::PhysicalPages),
        page_dimensions: all_page_dimensions,
        encrypted: doc.is_encrypted(),
        text_extraction_allowed: !doc.is_encrypted(),
        file_size,
        title: None,
    };

    Ok(DecodeResult {
        metadata,
        primitives,
        warnings,
        images,
    })
}

/// Extract raster images for a single PDF page onto the dedicated images
/// channel, preserving tagged-PDF alt-text bindings when available.
///
/// This is intentionally non-invasive — it does NOT touch `primitives`, so the
/// text / markdown / JSON outputs stay byte-identical to their pre-Brick-B
/// baselines. Images flow through
/// [`crate::api::Document::images`] / [`crate::api::Page::images`].
fn extract_page_images(
    doc: &mut PdfDocument,
    page_idx: usize,
    page_num: u32,
    page_dims: &PageDimensions,
    tagged_context: &super::tagged::TaggedPdfContext,
    warnings: &mut Vec<Warning>,
    images: &mut Vec<ExtractedImage>,
) {
    let pdf_images = match doc.extract_images(page_idx) {
        Ok(imgs) => imgs,
        Err(err) => {
            warnings.push(Warning {
                kind: crate::error::WarningKind::PartialExtraction,
                message: format!("Failed to extract images on this page: {}", err),
                page: Some(page_num),
            });
            return;
        }
    };

    if pdf_images.is_empty() {
        return;
    }

    let appearances = scan_page_image_appearances(doc, page_idx, warnings);
    let page_ctx = tagged_context.page_context(page_num);
    let raw_images: Vec<RawImage> = convert_images(
        &pdf_images,
        page_ctx,
        appearances.as_deref(),
        page_num,
        warnings,
    );

    for raw in raw_images {
        let bbox = page_dims.normalize_bbox(
            raw.raw_x,
            raw.raw_y,
            raw.raw_width,
            raw.raw_height,
            /* flip_y = */ true,
        );
        images.push(ExtractedImage {
            page: page_num,
            bbox,
            format: raw.format,
            data: raw.data,
            alt_text: raw.alt_text,
        });
    }
}
