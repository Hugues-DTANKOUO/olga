//! Internal types for the PDF decoder.

use crate::model::geometry::BoundingBox;
use crate::model::{TextDirection, ValueOrigin};

// ---------------------------------------------------------------------------
// Character span — group of characters sharing font properties
// ---------------------------------------------------------------------------

/// A raw character extracted from a PDF page.
///
/// These are grouped into text spans (runs of characters with the same font
/// and close horizontal proximity) before being emitted as Primitives.
#[derive(Debug, Clone)]
#[allow(dead_code)] // v2 fields (is_monospace, origin_y) reserved for later bricks
pub(crate) struct RawChar {
    /// The Unicode character (single codepoint from pdf_oxide).
    pub text: char,
    /// Raw bounding box in page coordinates (before normalization).
    pub raw_x: f32,
    pub raw_y: f32,
    pub raw_width: f32,
    pub raw_height: f32,
    /// Font name (subset prefix stripped).
    pub font_name: String,
    /// Font size in points.
    pub font_size: f32,
    /// Whether the font is bold (inferred from font name or weight).
    pub is_bold: bool,
    /// Whether the font is italic (inferred from font name or style flag).
    pub is_italic: bool,

    // --- v2 fields: full TextChar exploitation ---
    /// Text color RGB components (0.0-1.0 range, pdf_oxide native).
    /// Used for hidden text detection (white-on-white, zero-opacity).
    pub color_r: f32,
    pub color_g: f32,
    pub color_b: f32,
    /// Whether the font is monospaced (from PDF font descriptor FixedPitch flag).
    /// Useful for detecting code snippets, structured data, tabular content.
    pub is_monospace: bool,
    /// Baseline origin X coordinate (typographic origin, more precise than bbox.x).
    /// This is where the character's baseline starts in the PDF coordinate system.
    pub origin_x: f32,
    /// Baseline origin Y coordinate (typographic origin, more precise than bbox.y).
    pub origin_y: f32,
    /// Horizontal advance width: distance to the next character position.
    /// This is the typographic metric from the font Widths dictionary,
    /// more reliable than geometric gap (bbox-based) for word boundary
    /// detection. Advance-width over bbox gap is the convention used by
    /// mainstream PDF extractors (pdfminer.six, pdfplumber, PdfPig); see
    /// ATTRIBUTION.md.
    pub advance_width: f32,
    /// Per-character rotation in degrees (0-360, clockwise from horizontal).
    /// Computed from the text transformation matrix: atan2(b, a).
    /// 0 = normal, 90 = vertical top-to-bottom, 270 = vertical bottom-to-top.
    pub rotation_degrees: f32,
    /// Marked Content ID — links this character to a Tagged PDF structure tree node.
    /// Used by the structure engine for semantic hints (headings, paragraphs, etc.).
    pub mcid: Option<u32>,
}

/// A span of text (group of characters) ready for Primitive emission.
#[derive(Debug, Clone)]
pub(crate) struct TextSpan {
    /// Concatenated text content.
    pub content: String,
    /// Merged bounding box in normalized page coordinates.
    pub bbox: BoundingBox,
    /// Font name for this span.
    pub font_name: String,
    /// Font size in points.
    pub font_size: f32,
    /// Whether the font is bold.
    pub is_bold: bool,
    /// Whether the font is italic.
    pub is_italic: bool,
    /// Text color as (r, g, b) in 0-255 range (converted from pdf_oxide's 0.0-1.0).
    /// None if all characters had default black color.
    pub color: Option<crate::model::Color>,
    /// Direction inferred for this span from observed PDF character signals.
    pub text_direction: TextDirection,
    /// Provenance of the inferred text direction.
    pub text_direction_origin: ValueOrigin,
    /// Marked Content ID carried by all characters in the span when it maps
    /// cleanly to a single tagged PDF content node.
    pub mcid: Option<u32>,
    /// Per-character bounding boxes in normalized coordinates.
    /// INVARIANT: len(char_bboxes) == len(content.chars()) — including spaces.
    pub char_bboxes: Vec<(char, BoundingBox)>,
}

/// A raw vector path extracted from a PDF page.
#[derive(Debug, Clone)]
pub(crate) enum RawPath {
    /// A line segment.
    Line {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        thickness: f32,
        color_r: u8,
        color_g: u8,
        color_b: u8,
    },
    /// A rectangle (filled or stroked).
    Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        fill_r: Option<u8>,
        fill_g: Option<u8>,
        fill_b: Option<u8>,
        stroke_r: Option<u8>,
        stroke_g: Option<u8>,
        stroke_b: Option<u8>,
        stroke_thickness: f32,
    },
}

/// A raw image extracted from a PDF page.
#[derive(Debug, Clone)]
pub(crate) struct RawImage {
    /// Raw image data bytes.
    pub data: Vec<u8>,
    /// Image format (guessed from header bytes).
    pub format: crate::model::ImageFormat,
    /// Alt text bound from tagged PDF structure when the mapping is unique.
    pub alt_text: Option<String>,
    /// Marked Content ID carried by the image appearance when available.
    pub mcid: Option<u32>,
    /// Bounding box in page coordinates.
    pub raw_x: f32,
    pub raw_y: f32,
    pub raw_width: f32,
    pub raw_height: f32,
}

// ---------------------------------------------------------------------------
// Character grouping constants
// ---------------------------------------------------------------------------

/// Maximum horizontal gap (as fraction of font_size) before breaking into a new span.
///
/// When advance_width is available, the gap is computed as:
///   gap = ch.origin_x - (prev.origin_x + prev.advance_width)
/// When advance_width is zero (fallback), the gap is:
///   gap = ch.raw_x - (prev.raw_x + prev.raw_width)
///
/// Reference: PDFMiner uses char_margin=2.0 relative to char_width. Our threshold
/// is relative to font_size and more conservative because advance_width already
/// normalizes for glyph width variations.
///
/// Validated against PDFMiner/PdfPig reference implementations; refine with corpus testing.
pub(crate) const SPAN_BREAK_GAP_RATIO: f32 = 0.30;

/// Horizontal gap threshold (as fraction of font_size) to insert an inter-word space.
///
/// Reference: PDFMiner uses word_margin=0.1 relative to char_width.
/// PdfPig uses 20% threshold with Manhattan distance.
///
/// Validated against PDFMiner/PdfPig reference implementations; refine with corpus testing.
pub(crate) const WORD_GAP_RATIO: f32 = 0.15;

/// Maximum vertical offset (as fraction of font_size) for characters to be
/// considered on the same line.
///
/// Reference: PDFMiner uses line_overlap=0.5 relative to char_height.
/// Our threshold is more conservative — vertical alignment within 20% of font_size.
///
/// Validated against PDFMiner/PdfPig reference implementations; refine with corpus testing.
pub(crate) const LINE_DRIFT_RATIO: f32 = 0.20;

/// Leftward jump threshold (as fraction of font_size) that indicates a new line.
///
/// When a character appears significantly to the left of the previous character,
/// it likely starts a new line. This catches cases where vertical drift is small
/// but the horizontal position resets.
pub(crate) const NEWLINE_BACKTRACK_RATIO: f32 = 0.50;
