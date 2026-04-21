//! Cell construction and table grouping — Steps 4–5 of the table detection pipeline.
//!
//! Given a set of intersection points (from Step 3), this module:
//!
//! 1. **Constructs cells**: For each intersection, finds the smallest rectangle
//!    formed by 4 connected intersections (top-left, top-right, bottom-left,
//!    bottom-right). Edge connectivity is validated via pre-built spatial indexes
//!    (`HashSet` for intersection lookup, merged edge spans for coverage checks).
//!    Supports `Strict` (all 4 edges) and `Relaxed` (2 opposing edges, for
//!    stream/partial grid virtual edges).
//!
//! 2. **Groups cells into tables**: Union-Find O(n) connected-component analysis
//!    via corner-indexed HashMap. Each table is a contiguous grid with
//!    row/column indices.
//!
//! 3. **Assigns text to cells**: character-level assignment when
//!    `char_positions` is available (majority-vote: the cell containing the
//!    most characters wins). Falls back to primitive bounding-box midpoint when
//!    per-glyph positions are absent.
//!
//! 4. **Post-processing filters** (Tabula-inspired):
//!    - Remove tables that contain no text (graphic-only false positives).
//!    - Validate minimum table dimensions (`min_rows × min_columns`, default 2×2).

mod assignment;
mod construct;
mod grouping;
mod spatial;
#[cfg(test)]
mod tests;
mod types;

pub use assignment::assign_text_to_table;
pub use construct::construct_cells;
pub use grouping::group_into_tables;
pub use types::{Cell, Table};
