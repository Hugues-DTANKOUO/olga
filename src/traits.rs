//! Core pipeline traits.
//!
//! Each pipeline stage is defined by a trait with a default heuristic
//! implementation. New heuristic implementations can be plugged in without
//! breaking existing code.

use crate::error::{IdpResult, Warning};
use crate::model::{DocumentMetadata, ExtractedImage, Primitive};

/// A format decoder converts a document into a stream of [`Primitive`]s.
///
/// Each supported format (PDF, DOCX, XLSX, HTML) has its own decoder.
/// The decoder is responsible for:
/// - Detecting and validating the format
/// - Extracting visual primitives with bounding boxes
/// - Attaching semantic hints where the format provides them
/// - Extracting document metadata
pub trait FormatDecoder {
    /// Human-readable name of this decoder (e.g., "DOCX", "PDF").
    fn name(&self) -> &str;

    /// File extensions this decoder supports (e.g., &["docx", "docm"]).
    fn supported_extensions(&self) -> &[&str];

    /// Extract metadata without full processing.
    ///
    /// This is a lightweight pre-flight check: page count, encryption status,
    /// format validation. Should be fast (no full content extraction).
    fn metadata(&self, data: &[u8]) -> IdpResult<DocumentMetadata>;

    /// Decode the document into a stream of Primitives.
    ///
    /// Returns: (primitives, warnings).
    /// - Primitives are in source order (page, then position within page).
    /// - Warnings are non-fatal issues encountered during decoding.
    /// - Warnings are the canonical way recoverable fidelity loss is surfaced
    ///   to library consumers.
    ///
    /// The decoder MUST NOT panic on malformed input. All errors are either
    /// returned as `IdpError` (fatal) or collected as `Warning` (non-fatal).
    ///
    /// The public contract is still batch-oriented, but decoders should keep an
    /// incremental internal architecture so this method remains a thin
    /// collector over a future streaming pipeline.
    fn decode(&self, data: Vec<u8>) -> IdpResult<DecodeResult>;
}

/// The result of a successful decode operation.
#[derive(Debug)]
pub struct DecodeResult {
    /// Document metadata.
    pub metadata: DocumentMetadata,
    /// Extracted primitives in source order.
    pub primitives: Vec<Primitive>,
    /// Non-fatal warnings encountered during decoding.
    pub warnings: Vec<Warning>,
    /// Raster images extracted from the document, on a dedicated channel so
    /// they can be exposed without polluting the primitive stream or the
    /// text/markdown/JSON renderers.
    ///
    /// Populated by:
    /// - PDF: via `pdf_oxide::extract_images` + tagged-PDF alt binding.
    /// - DOCX / HTML: mirrored from each [`crate::model::PrimitiveKind::Image`]
    ///   already emitted by their decoder.
    /// - XLSX: empty in v1 (sheet-level image extraction is not wired yet).
    pub images: Vec<ExtractedImage>,
}
