//! Processability — a health-check report for a document.
//!
//! `Document::processability()` consolidates the signals the engine already
//! emits (encryption flag, decoder warnings, structure-engine heuristics,
//! content coverage) into one actionable report. Consumers who want to
//! decide "is this input worth sending to LLM extraction?" can look at a
//! single [`Health`] verdict or drill into the list of blockers and
//! degradations.
//!
//! # Severity model
//!
//! - **[`Health::Blocked`]** — at least one issue would prevent a useful
//!   downstream result. Today that's encryption without extraction rights,
//!   or a document that decoded to zero pages of text.
//! - **[`Health::Degraded`]** — the document processed, but decoding
//!   lowered fidelity. Heuristic structure, approximate pagination, and
//!   recoverable warnings all land here.
//! - **[`Health::Ok`]** — no blockers and no degradations worth surfacing.
//!
//! The report is deterministic given a document: each kind of issue is
//! emitted at most once, and the ordering is stable.

use serde::{Deserialize, Serialize};

use crate::error::{Warning, WarningKind};
use crate::model::{DocumentMetadata, PaginationBasis, ValueOrigin};

// ---------------------------------------------------------------------------
// Health — overall verdict
// ---------------------------------------------------------------------------

/// Overall health verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Health {
    /// No blockers and no surfaced degradations.
    Ok,
    /// Processable, but decoding lowered fidelity. See `degradations`.
    Degraded,
    /// Cannot be meaningfully processed. See `blockers`.
    Blocked,
}

// ---------------------------------------------------------------------------
// HealthIssue — one actionable signal
// ---------------------------------------------------------------------------

/// A single health signal surfaced by [`Processability`].
///
/// Issues are classified by the engine into `blockers` or `degradations`
/// on [`Processability`]. They stay deduplicated: each variant appears at
/// most once per report, with a `count` when appropriate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HealthIssue {
    /// The document is encrypted and extraction is not allowed.
    Encrypted,
    /// Decoding finished but produced zero pages of visible text.
    EmptyContent,
    /// The engine could not complete structuring — typically a downstream
    /// invariant broke during decode.
    DecodeFailed,
    /// `page_count` was not read from real pagination data and should be
    /// treated as an estimate.
    ApproximatePagination { basis: PaginationBasis },
    /// Some or all pages had structure inferred by heuristic detectors
    /// instead of format-derived hints. Includes the number of such pages.
    HeuristicStructure { pages: u32 },
    /// Parts of the input could not be extracted, but decoding continued.
    PartialExtraction { count: usize },
    /// Required package parts were missing (DOCX `styles.xml`, etc.).
    MissingPart { count: usize },
    /// Unresolved style IDs referenced from the body.
    UnresolvedStyle { count: usize },
    /// Unresolved relationship IDs (hyperlinks, embedded parts).
    UnresolvedRelationship { count: usize },
    /// Media payloads referenced by the package were not found in the ZIP.
    MissingMedia { count: usize },
    /// Content had to be truncated to stay within safety limits.
    TruncatedContent { count: usize },
    /// Content was malformed but recoverable.
    MalformedContent { count: usize },
    /// Content was skipped because it was recognised as a document artifact
    /// (header/footer noise, etc.).
    FilteredArtifact { count: usize },
    /// Likely artifact content that was kept unfiltered.
    SuspectedArtifact { count: usize },
    /// An element or structure was skipped / treated defensively.
    OtherWarnings { count: usize },
}

// ---------------------------------------------------------------------------
// Processability — the report
// ---------------------------------------------------------------------------

/// Health-check report for a document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Processability {
    /// Overall verdict.
    pub health: Health,
    /// `true` when nothing in `blockers` would prevent processing.
    ///
    /// This is a richer version of
    /// [`DocumentMetadata::is_processable`] that also considers decoder
    /// outcomes (an encrypted document is not processable; so is a
    /// document whose decoder ran but produced zero text).
    pub is_processable: bool,
    /// Issues that prevent the document from being processed usefully.
    pub blockers: Vec<HealthIssue>,
    /// Issues that lower fidelity without stopping processing.
    pub degradations: Vec<HealthIssue>,
    /// Total logical pages as reported by the decoder.
    pub pages_total: u32,
    /// Number of pages whose rendered text is non-empty (after trimming).
    pub pages_with_content: u32,
    /// Count of all non-fatal warnings collected during decode + structure.
    pub warning_count: usize,
}

impl Processability {
    /// `true` when `health == Health::Ok`.
    pub fn is_ok(&self) -> bool {
        matches!(self.health, Health::Ok)
    }

    /// `true` when `health == Health::Degraded`.
    pub fn is_degraded(&self) -> bool {
        matches!(self.health, Health::Degraded)
    }

    /// `true` when `health == Health::Blocked`.
    pub fn is_blocked(&self) -> bool {
        matches!(self.health, Health::Blocked)
    }
}

// ---------------------------------------------------------------------------
// Computation
// ---------------------------------------------------------------------------

/// Build a [`Processability`] report when the document is encrypted / not
/// processable at the metadata level. No decode is needed.
pub(crate) fn from_blocked_metadata(metadata: &DocumentMetadata) -> Processability {
    Processability {
        health: Health::Blocked,
        is_processable: false,
        blockers: vec![HealthIssue::Encrypted],
        degradations: Vec::new(),
        pages_total: metadata.page_count,
        pages_with_content: 0,
        warning_count: 0,
    }
}

/// Build a [`Processability`] report when decoding / structuring failed
/// after metadata had said the document was processable. Treat as blocked.
pub(crate) fn from_decode_failure(metadata: &DocumentMetadata) -> Processability {
    Processability {
        health: Health::Blocked,
        is_processable: false,
        blockers: vec![HealthIssue::DecodeFailed],
        degradations: Vec::new(),
        pages_total: metadata.page_count,
        pages_with_content: 0,
        warning_count: 0,
    }
}

/// Parameters for the full report — assembled by `Document::processability`.
pub(crate) struct Inputs<'a> {
    pub metadata: &'a DocumentMetadata,
    pub warnings: &'a [Warning],
    pub heuristic_pages: u32,
    pub pages_with_content: u32,
}

/// Build a full [`Processability`] report.
///
/// Assumes the caller has already run decode + structure successfully (i.e.
/// we're past the early-return paths in `from_blocked_metadata` and
/// `from_decode_failure`).
pub(crate) fn from_inputs(inputs: Inputs<'_>) -> Processability {
    let Inputs {
        metadata,
        warnings,
        heuristic_pages,
        pages_with_content,
    } = inputs;

    let counts = WarningCounts::from_warnings(warnings);

    let mut blockers = Vec::new();
    let mut degradations = Vec::new();

    // A decoder that finished with zero extracted text is a blocker even
    // when every individual warning is recoverable. Consumers shouldn't
    // silently consume an empty document thinking it's fine.
    if metadata.page_count > 0 && pages_with_content == 0 {
        blockers.push(HealthIssue::EmptyContent);
    }

    if metadata.page_count_provenance.origin != ValueOrigin::Observed {
        degradations.push(HealthIssue::ApproximatePagination {
            basis: metadata.page_count_provenance.basis,
        });
    }

    if heuristic_pages > 0 {
        degradations.push(HealthIssue::HeuristicStructure {
            pages: heuristic_pages,
        });
    }

    // Emit warning-derived degradations in a fixed order so the report is
    // deterministic across runs.
    if counts.partial_extraction > 0 {
        degradations.push(HealthIssue::PartialExtraction {
            count: counts.partial_extraction,
        });
    }
    if counts.missing_part > 0 {
        degradations.push(HealthIssue::MissingPart {
            count: counts.missing_part,
        });
    }
    if counts.unresolved_style > 0 {
        degradations.push(HealthIssue::UnresolvedStyle {
            count: counts.unresolved_style,
        });
    }
    if counts.unresolved_relationship > 0 {
        degradations.push(HealthIssue::UnresolvedRelationship {
            count: counts.unresolved_relationship,
        });
    }
    if counts.missing_media > 0 {
        degradations.push(HealthIssue::MissingMedia {
            count: counts.missing_media,
        });
    }
    if counts.truncated_content > 0 {
        degradations.push(HealthIssue::TruncatedContent {
            count: counts.truncated_content,
        });
    }
    if counts.malformed_content > 0 {
        degradations.push(HealthIssue::MalformedContent {
            count: counts.malformed_content,
        });
    }
    if counts.filtered_artifact > 0 {
        degradations.push(HealthIssue::FilteredArtifact {
            count: counts.filtered_artifact,
        });
    }
    if counts.suspected_artifact > 0 {
        degradations.push(HealthIssue::SuspectedArtifact {
            count: counts.suspected_artifact,
        });
    }
    // Fold the informational / already-covered kinds into "other" so the
    // per-kind warning total still adds up to `warning_count`, but we
    // don't emit a ProcessabilityIssue for signals that are either already
    // represented elsewhere (`ApproximatePagination`, `HeuristicInference`)
    // or are by-design benign (`TrackedChangeResolved`, `FilteredArtifact`
    // already handled above).
    let other = counts.other_degradations();
    if other > 0 {
        degradations.push(HealthIssue::OtherWarnings { count: other });
    }

    let health = if !blockers.is_empty() {
        Health::Blocked
    } else if !degradations.is_empty() {
        Health::Degraded
    } else {
        Health::Ok
    };

    Processability {
        health,
        is_processable: blockers.is_empty(),
        blockers,
        degradations,
        pages_total: metadata.page_count,
        pages_with_content,
        warning_count: warnings.len(),
    }
}

// ---------------------------------------------------------------------------
// Internal: per-kind warning tally
// ---------------------------------------------------------------------------

#[derive(Default)]
struct WarningCounts {
    // Directly surfaced as degradations:
    partial_extraction: usize,
    missing_part: usize,
    unresolved_style: usize,
    unresolved_relationship: usize,
    missing_media: usize,
    truncated_content: usize,
    malformed_content: usize,
    filtered_artifact: usize,
    suspected_artifact: usize,

    // Informational / already covered by metadata signals. Kept in `other`
    // so the total warning count still tallies, but not re-emitted as its
    // own `HealthIssue`.
    approximate_pagination: usize,
    heuristic_inference: usize,
    tracked_change: usize,
    unsupported_element: usize,
    unexpected_structure: usize,
}

impl WarningCounts {
    fn from_warnings(warnings: &[Warning]) -> Self {
        let mut c = Self::default();
        for w in warnings {
            match w.kind {
                WarningKind::PartialExtraction => c.partial_extraction += 1,
                WarningKind::MissingPart => c.missing_part += 1,
                WarningKind::UnresolvedStyle => c.unresolved_style += 1,
                WarningKind::UnresolvedRelationship => c.unresolved_relationship += 1,
                WarningKind::MissingMedia => c.missing_media += 1,
                WarningKind::TruncatedContent => c.truncated_content += 1,
                WarningKind::MalformedContent => c.malformed_content += 1,
                WarningKind::FilteredArtifact => c.filtered_artifact += 1,
                WarningKind::SuspectedArtifact => c.suspected_artifact += 1,
                WarningKind::ApproximatePagination => c.approximate_pagination += 1,
                WarningKind::HeuristicInference => c.heuristic_inference += 1,
                WarningKind::TrackedChangeResolved => c.tracked_change += 1,
                WarningKind::UnsupportedElement => c.unsupported_element += 1,
                WarningKind::UnexpectedStructure => c.unexpected_structure += 1,
            }
        }
        c
    }

    /// Informational kinds that don't deserve a dedicated [`HealthIssue`]
    /// variant but are still useful to surface so the user knows something
    /// besides the "big" categories tripped. Pagination / structure
    /// heuristics are intentionally excluded — they're already represented
    /// by `ApproximatePagination` and `HeuristicStructure`.
    fn other_degradations(&self) -> usize {
        self.tracked_change + self.unsupported_element + self.unexpected_structure
    }
}
