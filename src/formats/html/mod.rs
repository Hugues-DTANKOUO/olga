//! HTML format decoder.
//!
//! Parses HTML documents via `scraper` (wrapping html5ever, WHATWG-compliant) by:
//! 1. Parsing the full HTML into a DOM tree
//! 2. Walking the DOM depth-first
//! 3. Mapping semantic elements to Primitives with SemanticHints
//! 4. Skipping non-content elements (script, style, nav, etc.)
//!
//! Design: HTML is always single-page (page=0). Geometry stays synthetic
//! `DomFlow`, but the walker now respects a small rendered subset from HTML
//! attributes and inline CSS: visibility, directionality, whitespace,
//! list-style ordering, and table captions. Full CSS layout fidelity remains
//! out of scope for this brick. When the decoder recovers structure from CSS
//! `display:*` or generic container fallbacks instead of native semantic HTML
//! or ARIA, it emits explicit heuristic warnings.

mod render;
mod structure;
mod text;
mod types;
mod walker;

use crate::error::{IdpError, IdpResult};
use crate::model::*;
use crate::traits::{DecodeResult, FormatDecoder};

use walker::walk_dom;

const HTML_EXTENSIONS: &[&str] = &["html", "htm", "xhtml"];

/// Explicit operating mode for the HTML decoder.
///
/// The current foundation brick is not a browser engine. It implements a
/// constrained static-rendered subset while keeping geometry in synthetic
/// DOM-flow space so downstream code can distinguish semantic recovery from
/// physical layout fidelity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HtmlDecoderMode {
    /// Parse static HTML plus a constrained rendered subset (`display`,
    /// `visibility`, `direction`, `writing-mode`, `white-space`, list/table
    /// semantics), while emitting synthetic `DomFlow` geometry.
    StaticRenderedSubsetDomFlow,
}

// ---------------------------------------------------------------------------
// HtmlDecoder — the public entry point
// ---------------------------------------------------------------------------

/// HTML format decoder.
pub struct HtmlDecoder;

impl HtmlDecoder {
    pub fn new() -> Self {
        Self
    }

    pub const fn mode(&self) -> HtmlDecoderMode {
        HtmlDecoderMode::StaticRenderedSubsetDomFlow
    }
}

impl Default for HtmlDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatDecoder for HtmlDecoder {
    fn name(&self) -> &str {
        "HTML"
    }

    fn supported_extensions(&self) -> &[&str] {
        HTML_EXTENSIONS
    }

    fn metadata(&self, data: &[u8]) -> IdpResult<DocumentMetadata> {
        let html_str = std::str::from_utf8(data)
            .map_err(|e| IdpError::UnsupportedFormat(format!("Invalid UTF-8 in HTML: {e}")))?;

        let document = scraper::Html::parse_document(html_str);

        // Extract <title> if present.
        let title_selector = scraper::Selector::parse("title").ok();
        let title = title_selector.and_then(|sel| {
            document
                .select(&sel)
                .next()
                .map(|el| el.text().collect::<String>().trim().to_string())
                .filter(|t| !t.is_empty())
        });

        Ok(DocumentMetadata {
            format: DocumentFormat::Html,
            page_count: 1,
            page_count_provenance: PaginationProvenance::synthetic(
                PaginationBasis::SingleLogicalPage,
            ),
            page_dimensions: vec![],
            encrypted: false,
            text_extraction_allowed: true,
            file_size: data.len() as u64,
            title,
        })
    }

    fn decode(&self, data: Vec<u8>) -> IdpResult<DecodeResult> {
        let html_str = std::str::from_utf8(&data)
            .map_err(|e| IdpError::UnsupportedFormat(format!("Invalid UTF-8 in HTML: {e}")))?;

        let document = scraper::Html::parse_document(html_str);
        let mut primitives = Vec::new();
        let mut warnings = Vec::new();

        walk_dom(&document, &mut primitives, &mut warnings);

        // Extract <title> if present.
        let title_selector = scraper::Selector::parse("title").ok();
        let title = title_selector.and_then(|sel| {
            document
                .select(&sel)
                .next()
                .map(|el| el.text().collect::<String>().trim().to_string())
                .filter(|t| !t.is_empty())
        });

        let metadata = DocumentMetadata {
            format: DocumentFormat::Html,
            page_count: 1,
            page_count_provenance: PaginationProvenance::synthetic(
                PaginationBasis::SingleLogicalPage,
            ),
            page_dimensions: vec![],
            encrypted: false,
            text_extraction_allowed: true,
            file_size: data.len() as u64,
            title,
        };

        let images = mirror_images_from_primitives(&primitives);

        Ok(DecodeResult {
            metadata,
            primitives,
            warnings,
            images,
        })
    }
}

/// Mirror each `PrimitiveKind::Image` into the dedicated images channel.
///
/// HTML decoders emit image primitives via the walker; duplicating those
/// entries onto the images channel keeps the primitive stream (and every
/// downstream renderer) byte-identical to its pre-Brick-B baseline.
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

#[cfg(test)]
mod tests;
