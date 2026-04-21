//! Reading Order Resolver — per-page primitive reordering.
//!
//! Corrects the order of primitives within a single page so that they
//! follow the human reading flow. Three strategies are available:
//!
//! - **Tagged**: Trusts the existing order (source hints are authoritative).
//! - **ColumnAware**: XY-Cut spatial partitioning for multi-column layouts.
//! - **Simple**: Top-to-bottom, left-to-right fallback for trivial layouts.
//!
//! # Strategy selection
//!
//! The [`select_resolver`] function chooses a strategy based on signal
//! richness: if ≥ 80% of primitives carry format-derived hints, the
//! tagged resolver is used (no reordering needed). Otherwise, column
//! detection determines whether XY-Cut or simple ordering is appropriate.
//!
//! # Algorithm limitations
//!
//! - **XY-Cut is a greedy, single-pass algorithm.** It works well for
//!   regular column layouts (newspapers, academic papers) but can fail on
//!   complex layouts with sidebars, floating figures, or irregular grids.
//! - **Gap threshold sensitivity.** The `column_gap_threshold` must be
//!   tuned per-corpus. Too small → spurious column splits on word-spaced
//!   text. Too large → real columns merged into one.
//! - **No RTL column detection.** Column detection always scans left-to-
//!   right; only the emit order changes for RTL. A pure RTL document with
//!   columns ordered right-to-left in the source should still work, but
//!   mixed LTR/RTL column layouts are not supported.
//! - **Pages with only non-text primitives** (decorations, images) are
//!   handled via Y-then-X fallback ordering since column detection
//!   requires text intervals.

use crate::model::{Primitive, TextDirection};

// ---------------------------------------------------------------------------
// ReadingOrderResolver trait
// ---------------------------------------------------------------------------

/// A strategy for reordering primitives within a single page.
///
/// Implementations MUST be `Send + Sync` so that resolvers can be shared
/// across threads in future parallel-page processing.
pub trait ReadingOrderResolver: Send + Sync {
    /// Human-readable name (for debugging / provenance).
    fn name(&self) -> &str;

    /// Reorder the primitives **in place** for the given page.
    ///
    /// `page_width` is the normalized page width (always 1.0 in our
    /// coordinate system, but passed explicitly for clarity).
    fn reorder(&self, primitives: &mut [Primitive]);
}

// ---------------------------------------------------------------------------
// TaggedOrderResolver
// ---------------------------------------------------------------------------

/// Uses the existing source order — no reordering.
///
/// Appropriate when the document format (DOCX, HTML, tagged PDF) already
/// provides authoritative reading order through structural hints.
pub struct TaggedOrderResolver;

impl ReadingOrderResolver for TaggedOrderResolver {
    fn name(&self) -> &str {
        "tagged"
    }

    fn reorder(&self, _primitives: &mut [Primitive]) {
        // Hints are authoritative — source order IS reading order.
    }
}

// ---------------------------------------------------------------------------
// SimpleResolver
// ---------------------------------------------------------------------------

/// Top-to-bottom, left-to-right ordering (or right-to-left for RTL).
///
/// Suitable for single-column documents or when column detection
/// is not needed.
pub struct SimpleResolver {
    /// Primary text direction (affects column emit order).
    pub text_direction: TextDirection,
}

impl Default for SimpleResolver {
    fn default() -> Self {
        Self {
            text_direction: TextDirection::LeftToRight,
        }
    }
}

impl ReadingOrderResolver for SimpleResolver {
    fn name(&self) -> &str {
        "simple"
    }

    fn reorder(&self, primitives: &mut [Primitive]) {
        let rtl = matches!(self.text_direction, TextDirection::RightToLeft);
        primitives.sort_by(|a, b| {
            // Primary: top-to-bottom (Y ascending).
            let y_cmp = a
                .bbox
                .y
                .partial_cmp(&b.bbox.y)
                .unwrap_or(std::cmp::Ordering::Equal);
            if y_cmp != std::cmp::Ordering::Equal {
                return y_cmp;
            }
            // Secondary: left-to-right (LTR) or right-to-left (RTL).
            if rtl {
                b.bbox
                    .x
                    .partial_cmp(&a.bbox.x)
                    .unwrap_or(std::cmp::Ordering::Equal)
            } else {
                a.bbox
                    .x
                    .partial_cmp(&b.bbox.x)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
        });
    }
}

// ---------------------------------------------------------------------------
// ColumnAwareResolver (XY-Cut)
// ---------------------------------------------------------------------------

/// XY-Cut spatial partitioning for multi-column layouts.
///
/// # Algorithm
///
/// 1. Project all text primitive X-coordinates onto a horizontal axis.
/// 2. Find gaps in the projection wider than `column_gap_threshold`.
/// 3. Split primitives into columns at gap boundaries.
/// 4. Within each column: sort by Y (top-to-bottom), then by X.
/// 5. Emit columns in left-to-right order (or right-to-left for RTL).
///
/// Non-text primitives (lines, images, rectangles) are assigned to the
/// column whose horizontal span they overlap the most. If they don't
/// overlap any column, they are placed after all column content,
/// ordered by Y then X.
///
/// # Limitations
///
/// - Minimum column width is enforced (`MIN_COLUMN_WIDTH_FRAC`). Columns
///   narrower than this fraction of the page are merged with neighbours
///   to prevent noise from narrow decorative elements.
/// - Pages with no text primitives fall through to simple Y→X ordering.
pub struct ColumnAwareResolver {
    /// Minimum normalized gap width to consider a column split.
    /// Default: 0.03 (3% of page width).
    pub column_gap_threshold: f32,
    /// Primary text direction.
    pub text_direction: TextDirection,
}

/// Minimum fraction of page width a column must span to be considered real.
/// Columns narrower than this are noise (vertical rules, narrow decorations).
const MIN_COLUMN_WIDTH_FRAC: f32 = 0.05;

impl Default for ColumnAwareResolver {
    fn default() -> Self {
        Self {
            column_gap_threshold: 0.03,
            text_direction: TextDirection::LeftToRight,
        }
    }
}

impl ColumnAwareResolver {
    /// Detect column boundaries from text primitives.
    ///
    /// Returns a sorted list of `(left_edge, right_edge)` column extents.
    /// If no clear column structure is found, returns a single column
    /// spanning the full page width.
    fn detect_columns(&self, primitives: &[Primitive]) -> Vec<(f32, f32)> {
        // Collect horizontal extents of text primitives.
        let mut intervals: Vec<(f32, f32)> = primitives
            .iter()
            .filter(|p| p.is_text())
            .map(|p| (p.bbox.x, p.bbox.right()))
            .collect();

        if intervals.is_empty() {
            return vec![(0.0, 1.0)];
        }

        // Sort by left edge.
        intervals.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        // Merge overlapping intervals to find occupied bands.
        let mut merged: Vec<(f32, f32)> = Vec::new();
        for (left, right) in &intervals {
            if let Some(last) = merged.last_mut() {
                if *left <= last.1 + self.column_gap_threshold {
                    // Overlapping or within gap tolerance — extend.
                    last.1 = last.1.max(*right);
                } else {
                    // Gap found — new band.
                    merged.push((*left, *right));
                }
            } else {
                merged.push((*left, *right));
            }
        }

        if merged.len() < 2 {
            return vec![(0.0, 1.0)];
        }

        // Verify that gaps between merged bands are >= column_gap_threshold.
        let mut columns: Vec<(f32, f32)> = Vec::new();
        let mut current = merged[0];
        for band in &merged[1..] {
            let gap = band.0 - current.1;
            if gap >= self.column_gap_threshold {
                columns.push(current);
                current = *band;
            } else {
                // Gap too small — merge into current column.
                current.1 = current.1.max(band.1);
            }
        }
        columns.push(current);

        // Filter out noise columns that are too narrow.
        let columns: Vec<(f32, f32)> = columns
            .into_iter()
            .filter(|(l, r)| (r - l) >= MIN_COLUMN_WIDTH_FRAC)
            .collect();

        if columns.len() < 2 {
            vec![(0.0, 1.0)]
        } else {
            columns
        }
    }

    /// Assign a primitive to its best-fit column index, or `None` if it
    /// doesn't overlap any column meaningfully.
    fn assign_column(&self, prim: &Primitive, columns: &[(f32, f32)]) -> Option<usize> {
        let px = prim.bbox.x;
        let pr = prim.bbox.right();

        let mut best_idx = None;
        let mut best_overlap = 0.0_f32;

        for (i, &(cl, cr)) in columns.iter().enumerate() {
            let overlap_left = px.max(cl);
            let overlap_right = pr.min(cr);
            let overlap = (overlap_right - overlap_left).max(0.0);
            if overlap > best_overlap {
                best_overlap = overlap;
                best_idx = Some(i);
            }
        }

        // Require at least some meaningful overlap.
        if best_overlap > 0.0 { best_idx } else { None }
    }
}

impl ReadingOrderResolver for ColumnAwareResolver {
    fn name(&self) -> &str {
        "column-aware"
    }

    fn reorder(&self, primitives: &mut [Primitive]) {
        if primitives.len() <= 1 {
            return;
        }

        let columns = self.detect_columns(primitives);

        // Fast path: single column detected → fall back to simple sort.
        if columns.len() <= 1 {
            let simple = SimpleResolver {
                text_direction: self.text_direction,
            };
            simple.reorder(primitives);
            return;
        }

        let rtl = matches!(self.text_direction, TextDirection::RightToLeft);

        // Distribute primitives into column buckets.
        let n_cols = columns.len();
        let mut buckets: Vec<Vec<(usize, &Primitive)>> = vec![Vec::new(); n_cols];
        let mut unassigned: Vec<(usize, &Primitive)> = Vec::new();

        for (idx, prim) in primitives.iter().enumerate() {
            if let Some(col) = self.assign_column(prim, &columns) {
                buckets[col].push((idx, prim));
            } else {
                unassigned.push((idx, prim));
            }
        }

        // Sort within each bucket: top-to-bottom, then left-to-right (or RTL).
        for bucket in &mut buckets {
            bucket.sort_by(|a, b| {
                let y_cmp =
                    a.1.bbox
                        .y
                        .partial_cmp(&b.1.bbox.y)
                        .unwrap_or(std::cmp::Ordering::Equal);
                if y_cmp != std::cmp::Ordering::Equal {
                    return y_cmp;
                }
                if rtl {
                    b.1.bbox
                        .x
                        .partial_cmp(&a.1.bbox.x)
                        .unwrap_or(std::cmp::Ordering::Equal)
                } else {
                    a.1.bbox
                        .x
                        .partial_cmp(&b.1.bbox.x)
                        .unwrap_or(std::cmp::Ordering::Equal)
                }
            });
        }

        // Sort unassigned by Y then X.
        unassigned.sort_by(|a, b| {
            let y_cmp =
                a.1.bbox
                    .y
                    .partial_cmp(&b.1.bbox.y)
                    .unwrap_or(std::cmp::Ordering::Equal);
            if y_cmp != std::cmp::Ordering::Equal {
                return y_cmp;
            }
            a.1.bbox
                .x
                .partial_cmp(&b.1.bbox.x)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Build the desired ordering as a list of original indices.
        let mut order: Vec<usize> = Vec::with_capacity(primitives.len());

        // Emit columns in reading direction order.
        if rtl {
            for bucket in buckets.iter().rev() {
                order.extend(bucket.iter().map(|(idx, _)| *idx));
            }
        } else {
            for bucket in &buckets {
                order.extend(bucket.iter().map(|(idx, _)| *idx));
            }
        }

        // Append unassigned at the end.
        order.extend(unassigned.iter().map(|(idx, _)| *idx));

        // Apply the permutation in-place.
        apply_permutation(primitives, &order);
    }
}

// ---------------------------------------------------------------------------
// Strategy selection
// ---------------------------------------------------------------------------

/// Automatic strategy selection based on hint coverage.
///
/// Returns one of the three resolver strategies:
/// - `TaggedOrderResolver` if ≥ `hint_threshold` fraction of primitives have hints.
/// - `ColumnAwareResolver` if multi-column structure is detected.
/// - `SimpleResolver` otherwise.
pub fn select_resolver(
    primitives: &[Primitive],
    hint_threshold: f32,
    column_gap_threshold: f32,
) -> Box<dyn ReadingOrderResolver> {
    if primitives.is_empty() {
        return Box::new(TaggedOrderResolver);
    }

    // Check hint coverage.
    let hinted = primitives.iter().filter(|p| !p.hints.is_empty()).count();
    let coverage = hinted as f32 / primitives.len() as f32;

    if coverage >= hint_threshold {
        return Box::new(TaggedOrderResolver);
    }

    // Detect dominant text direction.
    let text_direction = detect_dominant_direction(primitives);

    // Check for multi-column structure.
    let resolver = ColumnAwareResolver {
        column_gap_threshold,
        text_direction,
    };
    let columns = resolver.detect_columns(primitives);

    if columns.len() >= 2 {
        Box::new(resolver)
    } else {
        Box::new(SimpleResolver { text_direction })
    }
}

/// Detect the dominant text direction from primitives.
fn detect_dominant_direction(primitives: &[Primitive]) -> TextDirection {
    let mut ltr = 0u32;
    let mut rtl = 0u32;

    for p in primitives {
        if let crate::model::PrimitiveKind::Text { text_direction, .. } = &p.kind {
            match text_direction {
                TextDirection::LeftToRight => ltr += 1,
                TextDirection::RightToLeft => rtl += 1,
                _ => {}
            }
        }
    }

    if rtl > ltr {
        TextDirection::RightToLeft
    } else {
        TextDirection::LeftToRight
    }
}

// ---------------------------------------------------------------------------
// Permutation helper
// ---------------------------------------------------------------------------

/// Apply a permutation to a mutable slice in-place.
///
/// `order[i]` is the index of the element that should end up at position `i`.
/// Runs in O(n) time with O(n) temporary storage (we clone into a new Vec
/// and copy back — primitives are not tiny, but pages are bounded, so this
/// is acceptable).
fn apply_permutation<T: Clone>(slice: &mut [T], order: &[usize]) {
    debug_assert_eq!(slice.len(), order.len());
    let cloned: Vec<T> = order.iter().map(|&i| slice[i].clone()).collect();
    for (i, item) in cloned.into_iter().enumerate() {
        slice[i] = item;
    }
}

#[cfg(test)]
#[path = "reading_order/tests.rs"]
mod tests;
