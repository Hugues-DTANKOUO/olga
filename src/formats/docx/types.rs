//! Internal types for the DOCX decoder.
//!
//! These types are used during parsing and are not part of the public API.

use crate::model::{Color, Primitive, TextDirection};

// ---------------------------------------------------------------------------
// Relationships
// ---------------------------------------------------------------------------

/// A resolved relationship from a .rels file.
#[derive(Debug, Clone)]
pub(crate) struct Relationship {
    /// Relationship type URI (truncated to the last segment for matching).
    pub rel_type: RelType,
    /// Target path (relative to the source part) or external URL.
    pub target: String,
    /// Whether the target is external (URL) vs internal (ZIP path).
    pub is_external: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum RelType {
    Image,
    Hyperlink,
    Header,
    Footer,
    Footnotes,
    Endnotes,
    Styles,
    Numbering,
    Comments,
    Other(String),
}

impl RelType {
    pub fn from_uri(uri: &str) -> Self {
        if uri.ends_with("/image") {
            Self::Image
        } else if uri.ends_with("/hyperlink") {
            Self::Hyperlink
        } else if uri.ends_with("/header") {
            Self::Header
        } else if uri.ends_with("/footer") {
            Self::Footer
        } else if uri.ends_with("/footnotes") {
            Self::Footnotes
        } else if uri.ends_with("/endnotes") {
            Self::Endnotes
        } else if uri.ends_with("/styles") {
            Self::Styles
        } else if uri.ends_with("/numbering") {
            Self::Numbering
        } else if uri.ends_with("/comments") {
            Self::Comments
        } else {
            Self::Other(uri.to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// Styles
// ---------------------------------------------------------------------------

/// Resolved style information (flattened inheritance).
#[derive(Debug, Clone, Default)]
pub(crate) struct ResolvedStyle {
    /// Style name (e.g., "Heading 1", "Normal").
    pub name: String,
    /// Style ID (e.g., "Heading1", "Normal").
    pub style_id: String,
    /// Font size in half-points (OOXML uses half-points).
    pub font_size_half_pt: Option<u32>,
    /// Font name.
    pub font_name: Option<String>,
    /// Bold.
    pub is_bold: Option<bool>,
    /// Italic.
    pub is_italic: Option<bool>,
    /// Color (hex RGB without #).
    pub color: Option<String>,
    /// If this is a heading, its outline level (0-indexed, so 0 = H1).
    pub outline_level: Option<u8>,
    /// Parent style ID for inheritance.
    pub based_on: Option<String>,
}

// ---------------------------------------------------------------------------
// Numbering
// ---------------------------------------------------------------------------

/// Resolved numbering definition.
#[derive(Debug, Clone)]
pub(crate) struct NumberingLevel {
    /// Whether this is an ordered list.
    pub is_ordered: bool,
}

// ---------------------------------------------------------------------------
// Parser context (accumulated during streaming)
// ---------------------------------------------------------------------------

/// Context accumulated while parsing a run (<w:r>).
#[derive(Debug, Clone, Default)]
pub(crate) struct RunContext {
    pub text: String,
    pub is_bold: bool,
    pub is_italic: bool,
    pub font_size: f32,
    pub font_name: String,
    pub color: Option<Color>,
    pub text_direction: TextDirection,
    pub hyperlink_url: Option<String>,
    /// BCP 47 language tag from `<w:lang>` (e.g., "fr-FR", "ar-SA").
    pub lang: Option<String>,
}

/// Paragraph alignment from `<w:jc>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ParagraphAlignment {
    #[default]
    Left,
    Center,
    Right,
    Both,
}

/// Context accumulated while parsing a paragraph (<w:p>).
#[derive(Debug, Clone, Default)]
pub(crate) struct ParagraphContext {
    pub style_id: Option<String>,
    pub num_id: Option<String>,
    pub num_level: Option<u8>,
    pub indent_level: u8,
    pub alignment: ParagraphAlignment,
    pub is_bidi: bool,
    pub story_id: Option<String>,
    pub footnote_refs: Vec<String>,
    pub comment_refs: Vec<String>,
    pub in_textbox: bool,
    pub runs: Vec<RunContext>,
    pub pending_media: Vec<Primitive>,
}

/// Context for table parsing.
#[derive(Debug, Clone)]
pub(crate) struct TableContext {
    pub current_row: u32,
    pub current_col: u32,
    /// Horizontal span of the current cell from `<w:gridSpan>`.
    pub current_cell_colspan: u32,
    /// Whether the current row is explicitly marked as a repeating table header row.
    pub current_row_is_header: bool,
}

impl TableContext {
    pub fn new() -> Self {
        Self {
            current_row: 0,
            current_col: 0,
            current_cell_colspan: 1,
            current_row_is_header: false,
        }
    }
}

/// Which part of the DOCX we're parsing (affects hint generation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum PartContext {
    Body,
    Header,
    Footer,
    Footnotes,
    Endnotes,
    Comments,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct DocxPackageIssueCounts {
    pub missing_media_payloads: u32,
    pub broken_numbering_defs: u32,
    pub malformed_relationship_parts: u32,
    pub missing_style_definitions: u32,
    pub circular_style_chains: u32,
    pub malformed_auxiliary_xml_files: u32,
    pub malformed_secondary_story_xml_parts: u32,
}
