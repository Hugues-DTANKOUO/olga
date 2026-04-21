//! Core primitive types for the Olga pipeline.
//!
//! These types represent the intermediate model between format decoders and the FSM.
//! Every format decoder produces a stream of [`Primitive`]s, optionally enriched with
//! [`SemanticHint`]s. The FSM consumes this stream and emits `DocumentNode`s.
//!
//! **Contract:** All coordinates are normalized to 0.0–1.0 relative to the effective
//! page dimensions (after rotation, using CropBox). Y=0.0 is the TOP of the page.
//! PDF coordinates are source-derived. DOCX and XLSX coordinates are normalized
//! reconstructed positions that preserve logical structure but not physical
//! layout fidelity. HTML coordinates are synthetic DOM-flow positions.

use serde::{Deserialize, Serialize};
use std::fmt;

use super::geometry::{BoundingBox, Point};
use super::hints::{HintSource, SemanticHint};
use super::provenance::{GeometryProvenance, GeometrySpace, ValueOrigin};

// ---------------------------------------------------------------------------
// Color
// ---------------------------------------------------------------------------

/// RGBA color (each component 0–255).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn black() -> Self {
        Self::rgb(0, 0, 0)
    }

    pub fn white() -> Self {
        Self::rgb(255, 255, 255)
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.a == 255 {
            write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            write!(
                f,
                "#{:02x}{:02x}{:02x}{:02x}",
                self.r, self.g, self.b, self.a
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Text direction
// ---------------------------------------------------------------------------

/// Text flow direction — affects reading order resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TextDirection {
    /// Left-to-right (Latin, Cyrillic, etc.)
    #[default]
    LeftToRight,
    /// Right-to-left (Arabic, Hebrew)
    RightToLeft,
    /// Mixed bidirectional content (requires Unicode UBA for correct ordering)
    Mixed,
    /// Top-to-bottom (CJK vertical text)
    TopToBottom,
}

impl fmt::Display for TextDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LeftToRight => write!(f, "LTR"),
            Self::RightToLeft => write!(f, "RTL"),
            Self::Mixed => write!(f, "BiDi"),
            Self::TopToBottom => write!(f, "TTB"),
        }
    }
}

// ---------------------------------------------------------------------------
// Image format
// ---------------------------------------------------------------------------

/// Known image formats that can be embedded in documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Gif,
    Bmp,
    Tiff,
    Svg,
    Webp,
    Unknown,
}

impl fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Png => write!(f, "PNG"),
            Self::Jpeg => write!(f, "JPEG"),
            Self::Gif => write!(f, "GIF"),
            Self::Bmp => write!(f, "BMP"),
            Self::Tiff => write!(f, "TIFF"),
            Self::Svg => write!(f, "SVG"),
            Self::Webp => write!(f, "WebP"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

// ---------------------------------------------------------------------------
// Character position (for per-glyph bounding boxes)
// ---------------------------------------------------------------------------

/// Per-character position within a text primitive.
/// Available when the format provides glyph-level bounding boxes (PDF via PDFium).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CharPosition {
    /// The character (as a Unicode scalar).
    pub ch: char,
    /// Bounding box of this character in normalized page coordinates.
    pub bbox: BoundingBox,
}

// ---------------------------------------------------------------------------
// Primitive — the atomic unit of the pipeline
// ---------------------------------------------------------------------------

/// The atomic visual element flowing through the Olga pipeline.
///
/// Every format decoder produces a stream of `Primitive`s. Each carries:
/// - A kind (text, line, rectangle, image) with format-specific data
/// - A normalized bounding box (position on page)
/// - The source page number
/// - A stable sort key for ordering
/// - Zero or more semantic hints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Primitive {
    /// What kind of visual element this is, with associated data.
    pub kind: PrimitiveKind,
    /// Normalized bounding box (0.0–1.0) on the page.
    pub bbox: BoundingBox,
    /// Source page number (0-indexed).
    pub page: u32,
    /// Monotonically increasing order in the source file. Used for stable sorting
    /// when spatial ordering is ambiguous.
    pub source_order: u64,
    /// Provenance for the primitive geometry.
    pub geometry_provenance: GeometryProvenance,
    /// Semantic annotations from the format decoder or heuristic detectors.
    /// Often empty for untagged PDF; tagged PDFs can emit format-derived hints.
    /// Rich for DOCX/HTML (hints come from the format structure).
    pub hints: Vec<SemanticHint>,
}

impl Primitive {
    /// Create a new primitive with no hints.
    pub fn new(
        kind: PrimitiveKind,
        bbox: BoundingBox,
        page: u32,
        source_order: u64,
        geometry_provenance: GeometryProvenance,
    ) -> Self {
        Self {
            kind,
            bbox,
            page,
            source_order,
            geometry_provenance,
            hints: Vec::new(),
        }
    }

    /// Create a primitive with observed physical-page geometry.
    pub fn observed(kind: PrimitiveKind, bbox: BoundingBox, page: u32, source_order: u64) -> Self {
        Self::new(
            kind,
            bbox,
            page,
            source_order,
            GeometryProvenance::observed_physical(),
        )
    }

    /// Create a primitive with reconstructed geometry.
    pub fn reconstructed(
        kind: PrimitiveKind,
        bbox: BoundingBox,
        page: u32,
        source_order: u64,
        space: GeometrySpace,
    ) -> Self {
        Self::new(
            kind,
            bbox,
            page,
            source_order,
            GeometryProvenance::reconstructed(space),
        )
    }

    /// Create a primitive with synthetic geometry.
    pub fn synthetic(
        kind: PrimitiveKind,
        bbox: BoundingBox,
        page: u32,
        source_order: u64,
        space: GeometrySpace,
    ) -> Self {
        Self::new(
            kind,
            bbox,
            page,
            source_order,
            GeometryProvenance::synthetic(space),
        )
    }

    /// Add a semantic hint to this primitive.
    pub fn with_hint(mut self, hint: SemanticHint) -> Self {
        self.hints.push(hint);
        self
    }

    /// Add multiple semantic hints to this primitive.
    pub fn with_hints(mut self, hints: Vec<SemanticHint>) -> Self {
        self.hints.extend(hints);
        self
    }

    /// Returns `true` if this primitive has any format-derived hints.
    pub fn has_format_hints(&self) -> bool {
        self.hints
            .iter()
            .any(|h| matches!(h.source, HintSource::FormatDerived))
    }

    /// Returns the text content if this is a Text primitive.
    pub fn text_content(&self) -> Option<&str> {
        match &self.kind {
            PrimitiveKind::Text { content, .. } => Some(content),
            _ => None,
        }
    }

    /// Returns `true` if this is a Text primitive.
    pub fn is_text(&self) -> bool {
        matches!(self.kind, PrimitiveKind::Text { .. })
    }

    /// Returns `true` if this is a Line primitive.
    pub fn is_line(&self) -> bool {
        matches!(self.kind, PrimitiveKind::Line { .. })
    }
}

impl fmt::Display for Primitive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            PrimitiveKind::Text { content, .. } => {
                let preview = if content.len() > 40 {
                    format!("{}…", &content[..40])
                } else {
                    content.clone()
                };
                write!(f, "Text({}) p{} {}", preview, self.page, self.bbox)
            }
            PrimitiveKind::Line { .. } => {
                write!(f, "Line p{} {}", self.page, self.bbox)
            }
            PrimitiveKind::Rectangle { .. } => {
                write!(f, "Rect p{} {}", self.page, self.bbox)
            }
            PrimitiveKind::Image { format, .. } => {
                write!(f, "Image({}) p{} {}", format, self.page, self.bbox)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// PrimitiveKind — variant data for each primitive type
// ---------------------------------------------------------------------------

/// The visual element type with associated data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PrimitiveKind {
    /// A run of text with font metadata.
    Text {
        /// The text content (Unicode NFC-normalized).
        content: String,
        /// Font size in points (as specified in the document, before normalization).
        font_size: f32,
        /// Font family name (e.g., "Arial", "Times New Roman").
        font_name: String,
        /// Whether the font is bold.
        is_bold: bool,
        /// Whether the font is italic.
        is_italic: bool,
        /// Text color (None if unknown or default black).
        color: Option<Color>,
        /// Per-character bounding boxes (when available, e.g., from PDF).
        char_positions: Option<Vec<CharPosition>>,
        /// Text flow direction.
        text_direction: TextDirection,
        /// Provenance of the text direction signal.
        text_direction_origin: ValueOrigin,
        /// BCP 47 language tag (e.g., "fr-FR", "en-US", "ar-SA").
        /// Extracted from document metadata when available (e.g., `w:lang` in DOCX).
        /// `None` means the language is unknown and must be inferred downstream.
        lang: Option<String>,
        /// Provenance of the language tag when present.
        lang_origin: Option<ValueOrigin>,
    },
    /// A line segment (explicit rule, border, etc.).
    Line {
        /// Start point in normalized coordinates.
        start: Point,
        /// End point in normalized coordinates.
        end: Point,
        /// Line thickness in normalized coordinates.
        thickness: f32,
        /// Line color.
        color: Option<Color>,
    },
    /// A filled or stroked rectangle.
    Rectangle {
        /// Fill color (None if not filled).
        fill: Option<Color>,
        /// Stroke/border color (None if no border).
        stroke: Option<Color>,
        /// Stroke thickness in normalized coordinates.
        stroke_thickness: Option<f32>,
    },
    /// An embedded image.
    Image {
        /// Image format.
        format: ImageFormat,
        /// Raw image bytes.
        #[serde(with = "serde_bytes_base64")]
        data: Vec<u8>,
        /// Alt text (from HTML alt attribute, DOCX description, etc.).
        alt_text: Option<String>,
    },
}

/// Custom serde module for base64-encoding image bytes in JSON.
mod serde_bytes_base64 {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        let hex = bytes
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();
        hex.serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        let hex = String::deserialize(deserializer)?;
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).map_err(serde::de::Error::custom))
            .collect()
    }
}
