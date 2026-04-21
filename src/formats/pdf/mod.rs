//! PDF format decoder.
//!
//! Extracts text (with per-character positions), vector paths (lines, rectangles),
//! and images from text-extractable PDFs using `pdf_oxide`.
//!
//! Key design notes:
//! - PDF coordinates have Y=0 at BOTTOM; we flip to Y=0 at TOP via PageDimensions.
//! - Characters are grouped into text spans based on font properties + proximity.
//! - Tagged PDFs contribute format-derived reading order and semantic hints when
//!   the structure tree is present and not marked suspect.
//! - Tagged-PDF text emission now qualifies mixed pages explicitly: spans that
//!   fall back to physical order and structure items that do not bind to text
//!   both surface as warnings instead of staying implicit.
//! - The tagged path now also builds an internal semantic graph, retains
//!   section-like boundaries as primitive hints when they can be anchored to
//!   text, and captures `ObjectRef` placeholders instead of silently discarding
//!   them.
//! - Vector paths are critical for downstream table detection.
//! - This decoder emits raw positioned Primitives — the structure engine adds
//!   structure later.
//! - Out of scope: scanned (image-only) PDFs, cross-page structure recovery,
//!   and full multi-column reading order reconstruction for untagged PDFs.

mod annotations;
mod artifact_image;
mod artifact_risk;
mod artifact_text;
mod decode;
mod language;
mod marked_content;
pub(crate) mod metadata;
pub(crate) mod pages;
mod paths;
mod resources;
pub(crate) mod tagged;
pub(crate) mod types;

use pdf_oxide::PdfDocument;

use crate::error::{IdpError, IdpResult};
use crate::model::DocumentMetadata;
use crate::traits::{DecodeResult, FormatDecoder};

use decode::decode_pdf;

const PDF_EXTENSIONS: &[&str] = &["pdf"];

// ---------------------------------------------------------------------------
// PdfDecoder — public entry point
// ---------------------------------------------------------------------------

/// PDF format decoder using pdf_oxide.
pub struct PdfDecoder;

impl FormatDecoder for PdfDecoder {
    fn name(&self) -> &str {
        "PDF"
    }

    fn supported_extensions(&self) -> &[&str] {
        PDF_EXTENSIONS
    }

    fn metadata(&self, data: &[u8]) -> IdpResult<DocumentMetadata> {
        let mut doc = PdfDocument::from_bytes(data.to_vec())
            .map_err(|e| IdpError::UnsupportedFormat(format!("Failed to open PDF: {}", e)))?;

        let page_count = doc.page_count().map_err(|e| IdpError::Decode {
            context: "PDF metadata".into(),
            message: format!("Failed to get page count: {}", e),
        })?;

        let mut warnings = Vec::new();
        let meta = metadata::extract_doc_metadata(&mut doc, data.len(), page_count, &mut warnings);
        Ok(meta)
    }

    fn decode(&self, data: Vec<u8>) -> IdpResult<DecodeResult> {
        decode_pdf(data)
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
