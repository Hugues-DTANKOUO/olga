//! Provenance types for decoder outputs.
//!
//! These types make an explicit distinction between observed, reconstructed,
//! estimated, and synthetic signals. Large downstream systems must be able to
//! tell whether a value comes from the source format, from deterministic
//! reconstruction, from a best-effort estimate, or from a placeholder layout.

use serde::{Deserialize, Serialize};
use std::fmt;

/// How a decoded value was obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ValueOrigin {
    /// Directly observed in the source format.
    Observed,
    /// Reconstructed from authoritative structure in the source format.
    Reconstructed,
    /// Estimated from partial evidence or secondary metadata.
    Estimated,
    /// Synthesized by the decoder to preserve flow or shape a fallback model.
    Synthetic,
}

impl fmt::Display for ValueOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Observed => write!(f, "observed"),
            Self::Reconstructed => write!(f, "reconstructed"),
            Self::Estimated => write!(f, "estimated"),
            Self::Synthetic => write!(f, "synthetic"),
        }
    }
}

/// What coordinate space a primitive bbox belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeometrySpace {
    /// Physical page coordinates derived from the source document.
    PhysicalPage,
    /// Logical page-like flow reconstructed from the source structure.
    LogicalPage,
    /// Logical spreadsheet grid coordinates.
    LogicalGrid,
    /// Synthetic DOM-flow coordinates.
    DomFlow,
}

impl fmt::Display for GeometrySpace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PhysicalPage => write!(f, "physical-page"),
            Self::LogicalPage => write!(f, "logical-page"),
            Self::LogicalGrid => write!(f, "logical-grid"),
            Self::DomFlow => write!(f, "dom-flow"),
        }
    }
}

/// Provenance for primitive geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GeometryProvenance {
    pub origin: ValueOrigin,
    pub space: GeometrySpace,
}

impl GeometryProvenance {
    pub const fn new(origin: ValueOrigin, space: GeometrySpace) -> Self {
        Self { origin, space }
    }

    pub const fn observed_physical() -> Self {
        Self::new(ValueOrigin::Observed, GeometrySpace::PhysicalPage)
    }

    pub const fn reconstructed(space: GeometrySpace) -> Self {
        Self::new(ValueOrigin::Reconstructed, space)
    }

    pub const fn synthetic(space: GeometrySpace) -> Self {
        Self::new(ValueOrigin::Synthetic, space)
    }
}

/// How a [`DocumentNode`](super::DocumentNode) was derived by the Structure Engine.
///
/// Every node in the output tree carries this provenance so downstream consumers
/// can distinguish format-guaranteed structure from heuristic guesses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StructureSource {
    /// Assembled from format-derived hints (DOCX, HTML, tagged PDF).
    /// Confidence is always 1.0.
    HintAssembled,
    /// Detected by a named heuristic detector.
    HeuristicDetected {
        /// Name of the detector (e.g., "ParagraphDetector", "HeadingDetector").
        detector: String,
        /// Detection confidence (0.0–1.0).
        confidence: f32,
    },
}

impl fmt::Display for StructureSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HintAssembled => write!(f, "hint-assembled"),
            Self::HeuristicDetected {
                detector,
                confidence,
            } => write!(f, "heuristic({}, {:.0}%)", detector, confidence * 100.0),
        }
    }
}

/// How the logical page count exposed by `DocumentMetadata` was derived.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PaginationBasis {
    /// Real physical pages from the source document.
    PhysicalPages,
    /// Workbook sheet count.
    WorkbookSheets,
    /// HTML-style single logical page.
    SingleLogicalPage,
    /// Rendered page-break markers embedded in the document body.
    RenderedPageBreakMarkers,
    /// Application-provided metadata such as `docProps/app.xml`.
    ApplicationMetadata,
    /// Explicit page-break markers seen in the document body.
    ExplicitBreaks,
    /// Conservative fallback when no stronger pagination signal is available.
    FallbackDefault,
}

impl fmt::Display for PaginationBasis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PhysicalPages => write!(f, "physical-pages"),
            Self::WorkbookSheets => write!(f, "workbook-sheets"),
            Self::SingleLogicalPage => write!(f, "single-logical-page"),
            Self::RenderedPageBreakMarkers => write!(f, "rendered-page-break-markers"),
            Self::ApplicationMetadata => write!(f, "application-metadata"),
            Self::ExplicitBreaks => write!(f, "explicit-breaks"),
            Self::FallbackDefault => write!(f, "fallback-default"),
        }
    }
}

/// Provenance for the `page_count` exposed in `DocumentMetadata`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PaginationProvenance {
    pub origin: ValueOrigin,
    pub basis: PaginationBasis,
}

impl PaginationProvenance {
    pub const fn new(origin: ValueOrigin, basis: PaginationBasis) -> Self {
        Self { origin, basis }
    }

    pub const fn observed(basis: PaginationBasis) -> Self {
        Self::new(ValueOrigin::Observed, basis)
    }

    pub const fn estimated(basis: PaginationBasis) -> Self {
        Self::new(ValueOrigin::Estimated, basis)
    }

    pub const fn synthetic(basis: PaginationBasis) -> Self {
        Self::new(ValueOrigin::Synthetic, basis)
    }
}
