//! Edge extraction, snapping, and merging — Steps 1–2 of the table detection pipeline.
//!
//! An **edge** is a horizontal or vertical line segment in normalized page
//! coordinates. Edges come from three sources:
//!
//! 1. **Line primitives** — PDF vector lines become edges directly. Lines
//!    thicker than `max_line_thickness` are rejected as decorative borders.
//! 2. **Rectangle primitives** — each rectangle produces 4 edges (top,
//!    bottom, left, right). Tiny rectangles and thick-stroked ones are
//!    filtered.
//! 3. **Text alignment** (stream/partial grid) — virtual edges inferred from
//!    text position clustering. This module does NOT implement text-based
//!    edges; that logic lives in the parent `TableDetector` which feeds
//!    pre-built edges into the pipeline.
//!
//! After extraction, edges are **snapped** (nearby parallel edges are averaged
//! to a single position via centroid clustering) and **merged** (overlapping
//! collinear edges are joined into one longer edge).
//!
//! # Cell validation modes
//!
//! [`CellValidation`] controls how strictly cells are validated in Step 4:
//! - `Strict` (default) — all 4 edges required. Used for pure lattice
//!   (all edges are explicit ruling lines).
//! - `Relaxed` — 2 opposing edges sufficient. Used for stream and partial grid
//!   paths where virtual edges are approximate.
//!
//! # Pipeline
//!
//! Three focused submodules, each owned by its own file:
//!
//! 1. [`types`] — [`CellValidation`], [`Edge`], [`Orientation`], [`EdgeConfig`]
//!    and their inherent impls. The vocabulary shared by the rest of the
//!    pipeline and by downstream consumers (`cells`, `lattice`, `stream`).
//! 2. [`extract`] — [`extract_edges`] reads `Line` / `Rectangle` primitives
//!    and emits axis-aligned edges in normalized coordinates.
//! 3. [`merge`] — [`snap_and_merge`] collapses nearby parallel edges to a
//!    shared position and joins overlapping collinear segments.

mod extract;
mod merge;
#[cfg(test)]
mod tests;
mod types;

pub use extract::extract_edges;
pub use merge::snap_and_merge;
pub use types::{CellValidation, Edge, EdgeConfig, Orientation};
