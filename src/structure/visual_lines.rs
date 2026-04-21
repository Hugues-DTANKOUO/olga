//! Visual Line Grouper — groups primitives into visual lines preserving spatial layout.
//!
//! # Philosophy
//!
//! Elements on the same visual line must appear on the same output line.
//! Instead of forcing structure (table, list, paragraph) onto every piece
//! of text, we first **preserve** the visual layout, then optionally layer
//! semantic structure on top.
//!
//! # Algorithm
//!
//! 1. Collect all text primitives on a page.
//! 2. Cluster them into **visual lines** by Y-overlap: two primitives
//!    share a line if their vertical extents overlap by more than 50%.
//! 3. Within each visual line, sort spans left-to-right by X position.
//! 4. Compute proportional character positions: map physical X coordinates
//!    to character columns using the page's average character width.
//!
//! # Output
//!
//! A `Vec<VisualLine>` per page, where each line carries its spans with
//! column positions suitable for proportional text rendering.

mod barriers;
mod grouping;
mod tableish;
#[cfg(test)]
mod tests;
mod types;

pub use grouping::{group_visual_lines, render_line};
pub use tableish::looks_like_table;
pub use types::{AlignedSpan, PageMetrics, VisualLine};
