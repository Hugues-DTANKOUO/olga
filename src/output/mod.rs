//! Output renderers.
//!
//! Three renderers are provided:
//!
//! - [`json`] — Structured JSON with bounding boxes, confidence, provenance.
//!   Designed for programmatic consumption by downstream pipelines that need a
//!   structured representation of the document.
//!
//! - [`spatial`] — Plain text with faithful spatial placement. Each word is
//!   placed at its exact geometric position from the PDF. No semantic markup.
//!
//! - [`markdown`] — Markdown with faithful spatial placement plus inline
//!   formatting (**bold**, *italic*, `[links](url)`). Same geometric approach
//!   as spatial, enriched with font and annotation metadata.

pub(crate) mod col_resolve;
pub mod json;
pub mod markdown;
pub mod md_tree;
pub mod prim_spatial;
pub(crate) mod row_cluster;
pub(crate) mod rules;
pub mod spatial;
pub mod text_tree;

/// Output format selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Text,
    Markdown,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json => write!(f, "json"),
            Self::Text => write!(f, "text"),
            Self::Markdown => write!(f, "markdown"),
        }
    }
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "json" => Ok(Self::Json),
            "text" | "txt" => Ok(Self::Text),
            "markdown" | "md" => Ok(Self::Markdown),
            other => Err(format!(
                "unknown format '{}' (expected: json, text, markdown/md)",
                other
            )),
        }
    }
}
