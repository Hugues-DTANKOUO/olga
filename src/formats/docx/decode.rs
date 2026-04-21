mod package;
mod pagination;

use std::io::Cursor;

use crate::error::{IdpError, IdpResult, Warning, WarningKind};
use crate::model::DocumentFormat;
use crate::model::DocumentMetadata;
use crate::model::{PaginationBasis, PaginationProvenance};
use crate::traits::DecodeResult;

use super::PATH_DOCUMENT_XML;
use super::body::parse_body_xml;
use super::numbering::parse_numbering;
use super::rels::{
    DocumentPageCounts, collect_media, estimate_page_count, estimate_page_count_with_warnings,
    parse_document_page_counts, parse_rels, read_zip_entry, read_zip_entry_opt,
};
use super::styles::parse_styles;
use super::types::{DocxPackageIssueCounts, PartContext};
use package::{
    append_docx_package_infrastructure_summaries, process_secondary_parts, rels_path_for_document,
    resolve_document_path,
};
use pagination::{docx_pagination_warning, resolve_docx_page_count};

const PATH_DOCUMENT2_XML: &str = "word/document2.xml";

pub(super) fn decode_docx_metadata(data: &[u8]) -> IdpResult<DocumentMetadata> {
    let cursor = Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor)?;

    let Some(doc_path) = resolve_document_path(&mut archive) else {
        return Err(IdpError::UnsupportedFormat(format!(
            "ZIP archive does not contain {PATH_DOCUMENT_XML}"
        )));
    };

    let app_page_count = estimate_page_count(&mut archive);
    let doc_counts = read_zip_entry_opt(&mut archive, doc_path)
        .and_then(|xml| parse_document_page_counts(&xml).ok())
        .unwrap_or_default();
    let (page_count, page_count_provenance) = resolve_docx_page_count(app_page_count, doc_counts);

    Ok(DocumentMetadata {
        format: DocumentFormat::Docx,
        page_count,
        page_count_provenance,
        page_dimensions: vec![],
        encrypted: false,
        text_extraction_allowed: true,
        file_size: data.len() as u64,
        title: None,
    })
}

pub(super) fn decode_docx(data: &[u8]) -> IdpResult<DecodeResult> {
    let cursor = Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor)?;

    let Some(doc_path) = resolve_document_path(&mut archive) else {
        return Err(IdpError::MissingPart(PATH_DOCUMENT_XML.to_string()));
    };

    let mut warnings = Vec::new();
    let mut package_issue_counts = DocxPackageIssueCounts::default();

    let doc_rels = parse_rels(
        &mut archive,
        rels_path_for_document(doc_path),
        &mut warnings,
        true,
        &mut package_issue_counts,
    );
    let styles = parse_styles(&mut archive, &mut warnings, &mut package_issue_counts);
    let numbering = parse_numbering(&mut archive, &mut warnings, &mut package_issue_counts);
    let media = collect_media(
        &mut archive,
        &doc_rels,
        &mut warnings,
        &mut package_issue_counts,
    );
    let app_page_count =
        estimate_page_count_with_warnings(&mut archive, &mut warnings, &mut package_issue_counts);

    let doc_xml = read_zip_entry(&mut archive, doc_path)?;
    let doc_page_counts = match parse_document_page_counts(&doc_xml) {
        Ok(counts) => counts,
        Err(e) => {
            warnings.push(Warning {
                kind: WarningKind::MalformedContent,
                message: format!("Failed to parse pagination markers in {}: {}", doc_path, e),
                page: None,
            });
            DocumentPageCounts::default()
        }
    };
    let mut primitives = Vec::new();
    let mut source_order: u64 = 0;
    let mut current_page: u32 = 0;
    let mut y_position: f32 = 0.0;

    let _ = parse_body_xml(
        &doc_xml,
        &styles,
        &numbering,
        &doc_rels,
        &media,
        &mut primitives,
        &mut source_order,
        &mut current_page,
        &mut y_position,
        &mut warnings,
        PartContext::Body,
    );

    process_secondary_parts(
        &mut archive,
        &doc_rels,
        &styles,
        &numbering,
        &mut primitives,
        &mut source_order,
        &mut warnings,
        &mut package_issue_counts,
    );

    append_docx_package_infrastructure_summaries(&mut warnings, &package_issue_counts);

    let detected_page_count = current_page + 1;
    let explicit_page_counts = if detected_page_count > 1 {
        DocumentPageCounts {
            explicit_pages: Some(detected_page_count),
            ..doc_page_counts
        }
    } else {
        doc_page_counts
    };
    let (resolver_page_count, resolver_provenance) =
        resolve_docx_page_count(app_page_count, explicit_page_counts);

    // Enforce the decoder-side invariant:
    //
    //     metadata.page_count >= max(p.page for p in primitives) + 1
    //
    // Every primitive carries the page index it was emitted on. That index
    // is not a hint — it is a structural property written by the body-XML
    // parser as it encounters `<w:br w:type="page"/>`. If the pagination
    // resolver returns a smaller value (typical when `docProps/app.xml`
    // under-reports — see memo.docx), we must still cover every page on
    // which we have actually placed content. Otherwise every downstream
    // consumer that iterates `0..metadata.page_count` (the text/markdown
    // renderers, the JSON by-page map, etc.) silently drops primitives
    // past the ceiling.
    //
    // This invariant owned at the decoder boundary means downstream code
    // can trust `metadata.page_count` and does not need to re-derive a max
    // from the primitives. (The `prim_spatial` renderer still keeps a
    // belt-and-suspenders defensive union; this invariant is what makes
    // that defence a no-op in practice.)
    let primitive_page_upper = primitives.iter().map(|p| p.page + 1).max().unwrap_or(0);
    let (page_count, page_count_provenance) = if primitive_page_upper > resolver_page_count {
        (
            primitive_page_upper,
            PaginationProvenance::estimated(PaginationBasis::ExplicitBreaks),
        )
    } else {
        (resolver_page_count, resolver_provenance)
    };

    if let Some(message) = docx_pagination_warning(
        page_count,
        page_count_provenance,
        app_page_count,
        explicit_page_counts,
    ) {
        warnings.push(Warning {
            kind: WarningKind::ApproximatePagination,
            message,
            page: None,
        });
    }

    let metadata = DocumentMetadata {
        format: DocumentFormat::Docx,
        page_count,
        page_count_provenance,
        page_dimensions: vec![],
        encrypted: false,
        text_extraction_allowed: true,
        file_size: data.len() as u64,
        title: None,
    };

    let images = mirror_images_from_primitives(&primitives);

    Ok(DecodeResult {
        metadata,
        primitives,
        warnings,
        images,
    })
}

/// Mirror each `PrimitiveKind::Image` into the dedicated images channel.
///
/// This keeps the primitive stream (and therefore every downstream renderer)
/// byte-identical to its pre-Brick-B baseline while still populating
/// `DecodeResult::images` for the public API.
fn mirror_images_from_primitives(
    primitives: &[crate::model::Primitive],
) -> Vec<crate::model::ExtractedImage> {
    use crate::model::{ExtractedImage, PrimitiveKind};

    primitives
        .iter()
        .filter_map(|p| match &p.kind {
            PrimitiveKind::Image {
                format,
                data,
                alt_text,
            } => Some(ExtractedImage {
                page: p.page,
                bbox: p.bbox,
                format: *format,
                data: data.clone(),
                alt_text: alt_text.clone(),
            }),
            _ => None,
        })
        .collect()
}
