//! Error types for Olga.

use std::fmt;

/// The main error type for Olga.
#[derive(Debug, thiserror::Error)]
pub enum IdpError {
    /// I/O error (file not found, permission denied, etc.).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// ZIP archive error (corrupt archive, missing entry, etc.).
    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),

    /// XML parsing error.
    #[error("XML parsing error: {0}")]
    Xml(#[from] quick_xml::Error),

    /// The document format is not supported or cannot be determined.
    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),

    /// The document is encrypted and text extraction is not allowed.
    #[error("Document is encrypted and text extraction is restricted")]
    EncryptedDocument,

    /// A required part is missing from the document (e.g., document.xml in DOCX).
    #[error("Missing required part: {0}")]
    MissingPart(String),

    /// A relationship ID could not be resolved.
    #[error("Unresolved relationship: {0}")]
    UnresolvedRelationship(String),

    /// A style reference could not be resolved.
    #[error("Unresolved style: {0}")]
    UnresolvedStyle(String),

    /// A generic decoding error with context.
    #[error("Decode error in {context}: {message}")]
    Decode { context: String, message: String },

    /// Invalid UTF-8 data encountered.
    #[error("Invalid UTF-8: {0}")]
    InvalidEncoding(String),
}

/// Result type alias for Olga operations.
pub type IdpResult<T> = Result<T, IdpError>;

/// A non-fatal warning emitted during processing.
///
/// Warnings don't stop processing. They are the library's canonical mechanism
/// for reporting recoverable fidelity loss to callers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    pub kind: WarningKind,
    pub message: String,
    /// Page number where the warning occurred (if applicable).
    pub page: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningKind {
    /// An optional part was missing and the decoder fell back to a reduced mode.
    MissingPart,
    /// A style ID referenced in the document was not found in styles.xml.
    UnresolvedStyle,
    /// A relationship ID could not be resolved.
    UnresolvedRelationship,
    /// A media file referenced in a relationship could not be found in the ZIP.
    MissingMedia,
    /// An unsupported element was encountered and skipped.
    UnsupportedElement,
    /// A tracked change was auto-resolved (insertions accepted, deletions ignored).
    TrackedChangeResolved,
    /// An element was encountered in an unexpected location.
    UnexpectedStructure,
    /// Content had to be truncated or clamped to preserve decoder safety.
    TruncatedContent,
    /// Part of the input could not be extracted, but decoding continued.
    PartialExtraction,
    /// Metadata such as page count is a best-effort approximation.
    ApproximatePagination,
    /// Structure or semantics were inferred from a fallback heuristic.
    HeuristicInference,
    /// Content was intentionally filtered as a document artifact (e.g. header/footer noise).
    FilteredArtifact,
    /// Heuristics detected likely artifact content that was left unfiltered.
    SuspectedArtifact,
    /// Malformed content that was recoverable but may indicate data loss.
    MalformedContent,
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.page {
            Some(p) => write!(f, "[p{}] {:?}: {}", p, self.kind, self.message),
            None => write!(f, "{:?}: {}", self.kind, self.message),
        }
    }
}
