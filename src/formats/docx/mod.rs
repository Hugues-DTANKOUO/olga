//! DOCX format decoder.
//!
//! Parses Office Open XML (OOXML / ECMA-376) documents by:
//! 1. Opening the ZIP archive
//! 2. Resolving relationships (images, hyperlinks)
//! 3. Resolving styles (inheritance chain flattening)
//! 4. Resolving numbering definitions (lists)
//! 5. Streaming through document.xml to emit Primitives with SemanticHints
//! 6. Processing secondary parts (headers, footers, footnotes, comments)
//!
//! Design: all parsing is done from in-memory buffers (no disk I/O after initial read).
//! quick-xml is used in pull-parser (streaming) mode.
//! Coordinates are reconstructed and normalized: they preserve reading order and
//! structural grouping, but they are not a faithful physical layout model.

mod body;
mod decode;
mod numbering;
mod rels;
mod styles;
mod types;

use crate::traits::{DecodeResult, FormatDecoder};

use decode::{decode_docx, decode_docx_metadata};

const DOCX_EXTENSIONS: &[&str] = &["docx", "docm"];

// ---------------------------------------------------------------------------
// Standard ZIP paths inside a DOCX archive
// ---------------------------------------------------------------------------
pub(crate) const PATH_DOCUMENT_XML: &str = "word/document.xml";
pub(crate) const PATH_STYLES_XML: &str = "word/styles.xml";
pub(crate) const PATH_NUMBERING_XML: &str = "word/numbering.xml";
pub(crate) const PATH_WORD_PREFIX: &str = "word/";

/// DOCX format decoder.
pub struct DocxDecoder;

impl DocxDecoder {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DocxDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatDecoder for DocxDecoder {
    fn name(&self) -> &str {
        "DOCX"
    }

    fn supported_extensions(&self) -> &[&str] {
        DOCX_EXTENSIONS
    }

    fn metadata(&self, data: &[u8]) -> crate::error::IdpResult<crate::model::DocumentMetadata> {
        decode_docx_metadata(data)
    }

    fn decode(&self, data: Vec<u8>) -> crate::error::IdpResult<DecodeResult> {
        decode_docx(&data)
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
