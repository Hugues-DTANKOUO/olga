//! PDF metadata extraction: page dimensions, encryption, version.
//!
//! This module centralizes all metadata extraction logic so that both
//! `metadata()` and `decode()` share the same source of truth for page
//! dimensions, encryption status, and document properties.
//!
//! Note: pdf_oxide's `PageInfo` / `get_page_info()` are gated behind the
//! `rendering` feature (heavy deps). We use `get_page_media_box()` instead,
//! which provides (x0, y0, x1, y1) of the MediaBox. CropBox and rotation
//! extraction can be added later via direct page dictionary access if needed.

use pdf_oxide::PdfDocument;

use crate::error::{Warning, WarningKind};
use crate::model::*;

// ---------------------------------------------------------------------------
// Page dimensions extraction
// ---------------------------------------------------------------------------

/// Extract page dimensions from a single page using `get_page_media_box()`.
///
/// Returns a `PageDimensions` built from the MediaBox. CropBox and rotation
/// are not available without the `rendering` feature on pdf_oxide, so they
/// default to None / 0 respectively.
///
/// Falls back to US Letter (612x792 pt) with a warning if extraction fails.
pub(crate) fn extract_page_dims(
    doc: &mut PdfDocument,
    page_idx: usize,
    warnings: &mut Vec<Warning>,
) -> PageDimensions {
    match doc.get_page_media_box(page_idx) {
        Ok((x0, y0, x1, y1)) => {
            let width = (x1 - x0).abs();
            let height = (y1 - y0).abs();
            PageDimensions::new(
                RawBox::new(x0.min(x1), y0.min(y1), width, height),
                None, // CropBox: requires page dictionary parsing (future enhancement)
                0,    // Rotation: requires page dictionary parsing (future enhancement)
            )
        }
        Err(_) => {
            warnings.push(Warning {
                kind: WarningKind::MalformedContent,
                message: format!(
                    "Page {}: failed to read page dimensions, falling back to US Letter (612x792 pt)",
                    page_idx
                ),
                page: Some(page_idx as u32),
            });
            PageDimensions::new(RawBox::new(0.0, 0.0, 612.0, 792.0), None, 0)
        }
    }
}

// ---------------------------------------------------------------------------
// Document-level metadata
// ---------------------------------------------------------------------------

/// Extract complete document metadata using available pdf_oxide APIs.
///
/// Uses: `page_count()`, `is_encrypted()`, and per-page `get_page_media_box()`
/// for dimensions.
pub(crate) fn extract_doc_metadata(
    doc: &mut PdfDocument,
    data_len: usize,
    page_count: usize,
    warnings: &mut Vec<Warning>,
) -> DocumentMetadata {
    // Collect page dimensions for all pages.
    let page_dimensions: Vec<PageDimensions> = (0..page_count)
        .map(|idx| extract_page_dims(doc, idx, warnings))
        .collect();

    // Encryption detection.
    let encrypted = doc.is_encrypted();

    DocumentMetadata {
        format: DocumentFormat::Pdf,
        page_count: page_count as u32,
        page_count_provenance: PaginationProvenance::observed(PaginationBasis::PhysicalPages),
        page_dimensions,
        encrypted,
        text_extraction_allowed: !encrypted,
        file_size: data_len as u64,
        title: None,
    }
}

#[cfg(test)]
#[path = "metadata/tests.rs"]
mod tests;
