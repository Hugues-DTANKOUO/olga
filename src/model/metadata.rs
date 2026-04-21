//! Document metadata types for pre-flight checks.

use serde::{Deserialize, Serialize};
use std::fmt;

use super::geometry::PageDimensions;
use super::provenance::PaginationProvenance;

// ---------------------------------------------------------------------------
// DocumentFormat
// ---------------------------------------------------------------------------

/// The source document format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DocumentFormat {
    Pdf,
    Docx,
    Xlsx,
    Html,
    /// Future: ODT, PPTX, RTF, etc.
    Other(u32),
}

impl fmt::Display for DocumentFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pdf => write!(f, "PDF"),
            Self::Docx => write!(f, "DOCX"),
            Self::Xlsx => write!(f, "XLSX"),
            Self::Html => write!(f, "HTML"),
            Self::Other(id) => write!(f, "Other({})", id),
        }
    }
}

// ---------------------------------------------------------------------------
// DocumentMetadata
// ---------------------------------------------------------------------------

/// Pre-flight metadata about a document, gathered before full processing.
///
/// Used to check permissions, estimate resource needs, and choose the decoder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentMetadata {
    /// Detected format.
    pub format: DocumentFormat,
    /// Logical page count as exposed by the decoder.
    ///
    /// Semantics by format:
    /// - PDF: real page count from the document.
    /// - XLSX: sheet count.
    /// - HTML: always 1.
    /// - DOCX: best-effort estimate; may rely on explicit page breaks or
    ///   document metadata instead of rendered pagination.
    pub page_count: u32,
    /// Provenance of `page_count`.
    pub page_count_provenance: PaginationProvenance,
    /// Per-page dimensions when the format can expose physical page geometry.
    ///
    /// Today this is primarily populated by PDF. Other formats either leave it
    /// empty or rely on reconstructed/synthetic positioning at the Primitive
    /// layer.
    pub page_dimensions: Vec<PageDimensions>,
    /// Whether the document is encrypted.
    pub encrypted: bool,
    /// Whether text extraction is allowed (relevant for encrypted PDFs).
    pub text_extraction_allowed: bool,
    /// File size in bytes.
    pub file_size: u64,
    /// Document title (from metadata, if available).
    pub title: Option<String>,
}

impl DocumentMetadata {
    /// Returns `true` if the document can be processed by Olga.
    pub fn is_processable(&self) -> bool {
        !self.encrypted || self.text_extraction_allowed
    }
}

impl fmt::Display for DocumentMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({} pages, {} bytes{})",
            self.format,
            self.page_count,
            self.file_size,
            if self.encrypted { ", encrypted" } else { "" }
        )
    }
}
