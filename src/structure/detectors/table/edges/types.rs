//! Edge types and configuration — the vocabulary shared by the rest of the
//! edges pipeline.
//!
//! An **edge** is a horizontal or vertical line segment in normalized [0,1]
//! page coordinates. [`CellValidation`] controls how strictly cells built
//! from edges are validated downstream (Step 4 of the table pipeline).

// ---------------------------------------------------------------------------
// Cell validation mode
// ---------------------------------------------------------------------------

/// Controls how strictly cell-edge connectivity is validated when constructing
/// cells from intersection corners (Step 4).
///
/// `Strict` requires all 4 edges to be present and is the default for pure
/// lattice tables (where every rule line is explicit). `Relaxed` accepts
/// cells with at least 2 opposing edges and is used whenever some edges are
/// inferred rather than drawn (stream mode, partial grids).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CellValidation {
    /// All 4 edges (top, bottom, left, right) must connect the corners.
    /// Default for the lattice path (explicit rule lines only).
    #[default]
    Strict,
    /// At least 2 opposing edges (top+bottom OR left+right) must be present.
    /// Used for stream and partial grid paths where virtual edges are approximate.
    Relaxed,
}

// ---------------------------------------------------------------------------
// Edge type
// ---------------------------------------------------------------------------

/// A horizontal or vertical line segment in normalized \[0,1\] page coordinates.
#[derive(Debug, Clone, Copy)]
pub struct Edge {
    pub orientation: Orientation,
    /// For horizontal edges: Y position. For vertical edges: X position.
    pub position: f32,
    /// Start of the segment (X for horizontal, Y for vertical).
    pub start: f32,
    /// End of the segment (X for horizontal, Y for vertical). Always ≥ start.
    pub end: f32,
}

/// Edge direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

impl Edge {
    /// Length of the edge segment.
    #[inline]
    pub fn length(&self) -> f32 {
        self.end - self.start
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Tolerance parameters for edge processing, expressed in normalized \[0,1\]
/// page coordinates.
///
/// Defaults are calibrated so that 0.005 in normalized coordinates ≈ 3 PDF
/// points on a standard letter-size page (612×792 pt); this is a common
/// choice across PDF layout libraries, and pdfplumber in particular uses
/// the same 3-point figure. Callers may override when page dimensions or
/// source characteristics differ. See ATTRIBUTION.md.
#[derive(Debug, Clone, Copy)]
pub struct EdgeConfig {
    /// How close two parallel edges must be (in their position axis) to be
    /// snapped to the same position. Default: 0.005.
    pub snap_tolerance: f32,
    /// How close endpoints must be for two collinear edges to be merged.
    /// Default: 0.005.
    pub join_tolerance: f32,
    /// Minimum edge length; shorter edges are discarded as noise.
    /// Default: 0.01 (≈ 6–8 pt on a typical page).
    pub min_edge_length: f32,
    /// Maximum line thickness to consider as a table rule (thicker lines
    /// are likely decorative borders or filled rectangles used as design
    /// elements). Default: 0.008 (≈ 5 pt).
    pub max_line_thickness: f32,
    /// Cell validation mode: `Strict` (all 4 edges) or `Relaxed` (2 opposing).
    /// Default: `Strict` for pure lattice. Stream and partial-grid paths
    /// override to `Relaxed`.
    pub cell_validation: CellValidation,
}

impl Default for EdgeConfig {
    fn default() -> Self {
        Self {
            snap_tolerance: 0.005,
            join_tolerance: 0.005,
            min_edge_length: 0.01,
            max_line_thickness: 0.008,
            cell_validation: CellValidation::default(),
        }
    }
}
