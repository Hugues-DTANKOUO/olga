//! Semantic hint types for the Olga pipeline.
//!
//! Hints annotate [`Primitive`]s with structural roles. Rich formats (DOCX, HTML)
//! provide hints directly; for PDF they are inferred by geometric/typographic
//! heuristics.

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// SemanticHint
// ---------------------------------------------------------------------------

/// A semantic annotation attached to a [`Primitive`](super::Primitive).
///
/// Hints tell the FSM what structural role this primitive plays.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticHint {
    /// What structural role this primitive plays.
    pub kind: HintKind,
    /// Confidence score (0.0–1.0).
    /// - 1.0 for format-derived hints (DOCX `<w:pStyle w:val="Heading1">`)
    /// - 0.7–0.9 for heuristic-inferred hints
    pub confidence: f32,
    /// Where this hint came from, including provenance identity.
    ///
    /// The detector name is embedded directly in the variant:
    /// - `FormatDerived` — from the document format (no detector needed).
    /// - `HeuristicInferred { detector }` — from a named heuristic detector.
    ///
    /// Carried through to [`StructureSource`](super::provenance::StructureSource)
    /// on the emitted [`DocumentNode`](super::DocumentNode).
    pub source: HintSource,
}

impl SemanticHint {
    /// Create a format-derived hint (confidence = 1.0).
    pub fn from_format(kind: HintKind) -> Self {
        Self {
            kind,
            confidence: 1.0,
            source: HintSource::FormatDerived,
        }
    }

    /// Create a heuristic-inferred hint with the given confidence and detector name.
    pub fn from_heuristic(kind: HintKind, confidence: f32, detector: &str) -> Self {
        Self {
            kind,
            confidence: confidence.clamp(0.0, 1.0),
            source: HintSource::HeuristicInferred {
                detector: detector.to_string(),
            },
        }
    }

    /// Returns the detector name, if this hint came from a heuristic detector.
    pub fn provenance_name(&self) -> Option<&str> {
        match &self.source {
            HintSource::FormatDerived => None,
            HintSource::HeuristicInferred { detector } => Some(detector),
        }
    }
}

impl fmt::Display for SemanticHint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}({:.0}%,{})",
            self.kind,
            self.confidence * 100.0,
            self.source
        )
    }
}

// ---------------------------------------------------------------------------
// HintKind
// ---------------------------------------------------------------------------

/// The structural role a primitive plays.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HintKind {
    // -- Block-level --
    /// A heading at a given level (1 = top-level, 6 = lowest).
    Heading { level: u8 },
    /// A body paragraph.
    Paragraph,
    /// An item in a list.
    ///
    /// The optional `list_group` assigns items to the same parent list.
    /// Consecutive items with the same group ID belong to the same `List`
    /// node. The assembler uses this to know when to open/close lists.
    ListItem {
        depth: u8,
        ordered: bool,
        list_group: Option<u32>,
    },
    /// A cell in a table.
    TableCell {
        row: u32,
        col: u32,
        rowspan: u32,
        colspan: u32,
    },
    /// A cell that continues a table from the previous page.
    ///
    /// Emitted by the [`TableDetector`] when it detects a table whose column
    /// positions match the currently open table from the previous page
    /// (via [`DetectionContext::open_table_columns`]). The assembler uses
    /// this to append rows to the existing table instead of opening a new one.
    ///
    /// `row` is local to this page (starting at 0). The assembler applies
    /// a row offset derived from the existing table's max row.
    TableCellContinuation {
        row: u32,
        col: u32,
        rowspan: u32,
        colspan: u32,
    },
    /// Records that a primitive is contained within a table cell at a given
    /// nesting depth, even if the primitive itself has a more specific role.
    /// Depth 0 = direct cell in the current table. Depth > 0 = ancestor cell.
    ContainedInTableCell { row: u32, col: u32, depth: u32 },
    /// Marks content extracted from a nested table that had to be flattened out
    /// of a parent table cell in the Primitive layer.
    FlattenedFromTableCell { row: u32, col: u32, depth: u32 },
    /// A header cell in a table.
    TableHeader { col: u32 },
    /// A blockquote.
    BlockQuote,
    /// A code block.
    CodeBlock,

    // -- Page-level --
    /// A recurring header at the top of pages.
    PageHeader,
    /// A recurring footer at the bottom of pages.
    PageFooter,
    /// A footnote reference/body.
    Footnote { id: String },
    /// A sidebar or marginal note.
    Sidebar,

    // -- Inline --
    /// A hyperlink.
    Link { url: String },
    /// Expanded text carried by a source format (for example tagged PDF `/E`).
    ExpandedText { text: String },
    /// Emphasis (italic) — informational, also in PrimitiveKind::Text.is_italic.
    Emphasis,
    /// Strong emphasis (bold) — informational.
    Strong,

    // -- Structure --
    /// Marks the beginning of a section.
    SectionStart { title: Option<String> },
    /// Marks the end of a section.
    SectionEnd,
    /// The document's title (often the first heading).
    DocumentTitle,
}

impl fmt::Display for HintKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Heading { level } => write!(f, "H{}", level),
            Self::Paragraph => write!(f, "Para"),
            Self::ListItem {
                depth,
                ordered,
                list_group,
            } => {
                write!(
                    f,
                    "Li(d={},{},g={})",
                    depth,
                    if *ordered { "ol" } else { "ul" },
                    list_group.map_or("-".to_string(), |g| g.to_string())
                )
            }
            Self::TableCell { row, col, .. } => write!(f, "Td({},{})", row, col),
            Self::TableCellContinuation { row, col, .. } => {
                write!(f, "TdCont({},{})", row, col)
            }
            Self::ContainedInTableCell { row, col, depth } => {
                write!(f, "InTd({},{},d={})", row, col, depth)
            }
            Self::FlattenedFromTableCell { row, col, depth } => {
                write!(f, "FlatTd({},{},d={})", row, col, depth)
            }
            Self::TableHeader { col } => write!(f, "Th({})", col),
            Self::BlockQuote => write!(f, "BQ"),
            Self::CodeBlock => write!(f, "Code"),
            Self::PageHeader => write!(f, "PgHdr"),
            Self::PageFooter => write!(f, "PgFtr"),
            Self::Footnote { id } => write!(f, "Fn({})", id),
            Self::Sidebar => write!(f, "Side"),
            Self::Link { url } => write!(f, "Link({})", url),
            Self::ExpandedText { text } => write!(f, "Expand({})", text),
            Self::Emphasis => write!(f, "Em"),
            Self::Strong => write!(f, "Strong"),
            Self::SectionStart { title } => match title {
                Some(t) => write!(f, "SecStart({})", t),
                None => write!(f, "SecStart"),
            },
            Self::SectionEnd => write!(f, "SecEnd"),
            Self::DocumentTitle => write!(f, "DocTitle"),
        }
    }
}

// ---------------------------------------------------------------------------
// HintSource
// ---------------------------------------------------------------------------

/// Where a semantic hint originated.
///
/// Each variant carries its provenance identity so the hint can be traced
/// back to the specific detector that produced it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HintSource {
    /// Directly from the document format (DOCX style, HTML element, etc.).
    /// Confidence is always 1.0.
    FormatDerived,
    /// Inferred by a named geometric/typographic heuristic detector.
    HeuristicInferred {
        /// Name of the detector (e.g., `"ParagraphDetector"`, `"HeadingDetector"`).
        detector: String,
    },
}

impl fmt::Display for HintSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FormatDerived => write!(f, "format"),
            Self::HeuristicInferred { detector } => write!(f, "heuristic({})", detector),
        }
    }
}
