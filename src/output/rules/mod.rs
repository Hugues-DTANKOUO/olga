//! Extraction and grid classification of horizontal/vertical line primitives
//! from a PDF page, shared by the spatial and markdown renderers.
//!
//! # Philosophy
//!
//! Pure geometric preservation, no semantic interpretation. Every stroked
//! line or rectangle edge in the PDF becomes a candidate segment. Segments
//! are classified as horizontal, vertical, or oblique based solely on their
//! projected extent on the render grid (Δcells < 0.5 collapses to an axis).
//! Oblique segments are dropped because the current ASCII palette only
//! contains `-` and `|`.
//!
//! No junctions, no corner characters, no dedicated rendering for special
//! shapes. When a horizontal rule and a vertical rule share a cell, the
//! last-written one wins. When a word shares a cell with a rule, the word
//! wins because words are drawn after rules.
//!
//! Fill-only rectangles are ignored: a filled rectangle is a filled area,
//! not a line, and ASCII has no semantically-neutral way to represent fills.
//!
//! # Pipeline
//!
//! Four focused stages, each owned by its own submodule:
//!
//! 1. [`pdf_segments`] — read `extract_lines` / `extract_rects` from the PDF
//!    and return [`PdfSegment`]s in raw PDF coordinates (Y-up, points).
//! 2. [`normalize`] — apply the word-placement `(min_x, min_y, cw, ch)`
//!    bounds to emit [`RawSegment`]s in `[0, 1]` normalized coordinates
//!    with the Y axis flipped (0 = top).
//! 3. [`snap`] — classify each normalized segment as horizontal, vertical,
//!    or oblique (dropped) and snap the orthogonal ones to cell indices.
//! 4. [`adjust`] — reconcile snapped rules with rows already occupied by
//!    text, making sure connected verticals inherit any horizontal shift.
//!
//! Bounds are computed by the caller by taking the per-axis min/max over
//! both words AND raw segments — see [`pdf_segments_bounds`]. Expanding
//! bounds to include rules guarantees that rules never fall outside the
//! grid and stay in the same coordinate frame as words.

mod adjust;
mod normalize;
mod pdf_segments;
mod snap;

#[cfg(test)]
mod tests;

pub(crate) use adjust::adjust_rules;
pub(crate) use normalize::{RawSegment, normalize_segments};
pub(crate) use pdf_segments::{extract_pdf_segments, pdf_segments_bounds};
pub(crate) use snap::{GridRule, snap_to_grid};
