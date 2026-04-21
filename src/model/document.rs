//! Output document model — the structured tree emitted by the FSM.
//!
//! The FSM consumes a stream of [`Primitive`]s and emits a stream of
//! [`DocumentNode`]s forming a hierarchical tree. Each node represents a
//! detected structural element (heading, paragraph, table, etc.) with its
//! children.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Range;

/// Truncate a string to at most `max_chars` Unicode characters.
///
/// Unlike `&text[..n]` which operates on bytes and can panic on multi-byte
/// characters (é, –, 中, etc.), this always splits at a character boundary.
fn truncate_chars(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        text.to_string()
    } else {
        let truncated: String = text.chars().take(max_chars).collect();
        format!("{}…", truncated)
    }
}

use super::geometry::BoundingBox;
use super::provenance::StructureSource;

// ---------------------------------------------------------------------------
// DocumentNode — the output tree
// ---------------------------------------------------------------------------

/// A node in the structured document tree.
///
/// Emitted by the FSM as it closes detected structural objects. The tree is
/// built incrementally: child nodes are emitted and collected before the parent
/// is finalized.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentNode {
    /// What structural element this node represents.
    pub kind: NodeKind,
    /// Bounding box spanning all contained primitives/children.
    pub bbox: BoundingBox,
    /// Classification confidence (0.0–1.0).
    /// - 1.0 for format-derived structure (DOCX, HTML)
    /// - Lower for heuristic-inferred structure (PDF)
    pub confidence: f32,
    /// How this node's structure was determined.
    ///
    /// Tells downstream consumers whether the structure came from the document
    /// format itself or from a heuristic detector.
    pub structure_source: StructureSource,
    /// Pages this node spans (e.g., 3..6 means pages 3, 4, 5).
    pub source_pages: Range<u32>,
    /// Child nodes (e.g., TableRows inside a Table, ListItems inside a List).
    pub children: Vec<DocumentNode>,
    /// Arbitrary metadata (format-specific attributes, style info, etc.).
    pub metadata: std::collections::HashMap<String, String>,
}

impl DocumentNode {
    /// Create a new document node with no children or metadata.
    pub fn new(
        kind: NodeKind,
        bbox: BoundingBox,
        confidence: f32,
        structure_source: StructureSource,
        source_pages: Range<u32>,
    ) -> Self {
        Self {
            kind,
            bbox,
            confidence,
            structure_source,
            source_pages,
            children: Vec::new(),
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Add a child node.
    pub fn add_child(&mut self, child: DocumentNode) {
        self.children.push(child);
    }

    /// Add metadata key-value pair.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Returns `true` if this node spans multiple pages.
    pub fn is_cross_page(&self) -> bool {
        self.source_pages.end - self.source_pages.start > 1
    }

    /// Returns the text content of this node (for leaf nodes).
    pub fn text(&self) -> Option<&str> {
        match &self.kind {
            NodeKind::Heading { text, .. } => Some(text),
            NodeKind::Paragraph { text } => Some(text),
            NodeKind::TableCell { text, .. } => Some(text),
            NodeKind::ListItem { text } => Some(text),
            NodeKind::CodeBlock { text, .. } => Some(text),
            NodeKind::BlockQuote { text } => Some(text),
            NodeKind::PageHeader { text } => Some(text),
            NodeKind::PageFooter { text } => Some(text),
            NodeKind::Footnote { text, .. } => Some(text),
            _ => None, // AlignedLine: use all_text() for concatenated spans.
        }
    }

    /// Recursively collect all text content from this node and its children.
    pub fn all_text(&self) -> String {
        let mut result = String::new();
        self.collect_text(&mut result);
        result
    }

    fn collect_text(&self, buf: &mut String) {
        if let NodeKind::AlignedLine { spans } = &self.kind {
            // Concatenate all span texts with spaces.
            for span in spans {
                if !buf.is_empty() {
                    buf.push(' ');
                }
                buf.push_str(&span.text);
            }
        } else if let Some(text) = self.text() {
            if !buf.is_empty() {
                buf.push(' ');
            }
            buf.push_str(text);
        }
        for child in &self.children {
            child.collect_text(buf);
        }
    }

    /// Count the total number of nodes in this subtree (including self).
    pub fn node_count(&self) -> usize {
        1 + self.children.iter().map(|c| c.node_count()).sum::<usize>()
    }

    /// Depth of the tree rooted at this node (leaf = 1).
    pub fn depth(&self) -> usize {
        if self.children.is_empty() {
            1
        } else {
            1 + self.children.iter().map(|c| c.depth()).max().unwrap_or(0)
        }
    }
}

impl fmt::Display for DocumentNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_indented(f, 0)
    }
}

impl DocumentNode {
    fn fmt_indented(&self, f: &mut fmt::Formatter<'_>, indent: usize) -> fmt::Result {
        let prefix = "  ".repeat(indent);
        let pages = if self.is_cross_page() {
            format!("p{}-{}", self.source_pages.start, self.source_pages.end - 1)
        } else {
            format!("p{}", self.source_pages.start)
        };
        let conf = if self.confidence < 1.0 {
            format!(" ({:.0}%)", self.confidence * 100.0)
        } else {
            String::new()
        };
        writeln!(f, "{}{} {}{}", prefix, self.kind, pages, conf)?;
        for child in &self.children {
            child.fmt_indented(f, indent + 1)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// NodeKind — structural element types
// ---------------------------------------------------------------------------

/// The type of structural element a [`DocumentNode`] represents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NodeKind {
    /// Root node of the document tree.
    Document,
    /// A section (logical grouping of content under a heading).
    Section { level: u8 },
    /// A heading with its text content.
    Heading { level: u8, text: String },
    /// A paragraph with its text content.
    Paragraph { text: String },
    /// A table with its dimensions.
    Table { rows: u32, cols: u32 },
    /// A row in a table.
    TableRow,
    /// A cell in a table with its position and text.
    TableCell {
        row: u32,
        col: u32,
        rowspan: u32,
        colspan: u32,
        text: String,
    },
    /// A list (ordered or unordered).
    List { ordered: bool },
    /// An item in a list.
    ListItem { text: String },
    /// An embedded image.
    Image {
        alt_text: Option<String>,
        /// Raw image bytes are stored in metadata to keep NodeKind Clone-friendly.
        /// Access via `DocumentNode.metadata["image_data_hex"]`.
        format: String,
    },
    /// A code block.
    CodeBlock {
        language: Option<String>,
        text: String,
    },
    /// A blockquote.
    BlockQuote { text: String },
    /// A page header (recurring).
    PageHeader { text: String },
    /// A page footer (recurring).
    PageFooter { text: String },
    /// A footnote.
    Footnote { id: String, text: String },
    /// A layout-preserving line with proportionally-spaced spans.
    ///
    /// Each span carries its character column position and optional inline
    /// formatting (bold, italic, hyperlink). This preserves both the visual
    /// alignment and the typographic emphasis from the source document.
    AlignedLine { spans: Vec<InlineSpan> },
}

/// An inline text span with formatting metadata.
///
/// Used by [`NodeKind::AlignedLine`] to carry per-primitive formatting
/// through to the renderers (e.g., bold → `**text**` in Markdown).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InlineSpan {
    /// Character column position (0-based) for proportional layout.
    pub col_start: usize,
    /// The text content.
    pub text: String,
    /// Whether this span is bold.
    pub is_bold: bool,
    /// Whether this span is italic.
    pub is_italic: bool,
    /// Hyperlink URL, if this span is a link.
    pub link_url: Option<String>,
}

impl fmt::Display for NodeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Document => write!(f, "Document"),
            Self::Section { level } => write!(f, "Section(L{})", level),
            Self::Heading { level, text } => {
                let preview = truncate_chars(text, 30);
                write!(f, "H{}({})", level, preview)
            }
            Self::Paragraph { text } => {
                let preview = truncate_chars(text, 30);
                write!(f, "Para({})", preview)
            }
            Self::Table { rows, cols } => write!(f, "Table({}x{})", rows, cols),
            Self::TableRow => write!(f, "TR"),
            Self::TableCell { row, col, text, .. } => {
                let preview = truncate_chars(text, 20);
                write!(f, "TD({},{}: {})", row, col, preview)
            }
            Self::List { ordered } => {
                write!(f, "{}List", if *ordered { "O" } else { "U" })
            }
            Self::ListItem { text } => {
                let preview = truncate_chars(text, 30);
                write!(f, "LI({})", preview)
            }
            Self::Image { alt_text, format } => {
                write!(
                    f,
                    "Img({}, {})",
                    format,
                    alt_text.as_deref().unwrap_or("no alt")
                )
            }
            Self::CodeBlock { language, .. } => {
                write!(f, "Code({})", language.as_deref().unwrap_or("plain"))
            }
            Self::BlockQuote { .. } => write!(f, "BQ"),
            Self::PageHeader { .. } => write!(f, "PgHdr"),
            Self::PageFooter { .. } => write!(f, "PgFtr"),
            Self::Footnote { id, .. } => write!(f, "Fn({})", id),
            Self::AlignedLine { spans } => {
                write!(f, "Aligned({}spans)", spans.len())
            }
        }
    }
}
