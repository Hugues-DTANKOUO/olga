//! Standalone image data channel.
//!
//! Images live on their own channel (see [`crate::traits::DecodeResult::images`])
//! rather than being injected into the primitive stream. Keeping raster assets
//! out of the primitive flow lets the structure engine, spatial renderers, and
//! text/markdown exports stay focused on layout and prose — but still exposes
//! the underlying bytes to consumers through
//! [`crate::api::Document::images`] and [`crate::api::Page::images`].

use crate::model::geometry::BoundingBox;
use crate::model::primitives::ImageFormat;

/// A raster image extracted from a document.
///
/// The `page` field is 0-indexed, matching [`crate::model::Primitive::page`];
/// the public API translates it to a 1-based number when filtering per page.
#[derive(Debug, Clone)]
pub struct ExtractedImage {
    /// Source page number (0-indexed).
    pub page: u32,
    /// Bounding box on the page (normalized 0.0–1.0).
    pub bbox: BoundingBox,
    /// Image format, inferred from the source container or magic bytes.
    pub format: ImageFormat,
    /// Raw image bytes, ready to be written to disk or re-encoded.
    pub data: Vec<u8>,
    /// Alt text when the source document provides one (tagged PDF / DOCX
    /// description / HTML `alt` attribute).
    pub alt_text: Option<String>,
}
