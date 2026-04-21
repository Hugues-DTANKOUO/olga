//! Page Buffer — collects primitives for one page, then flushes.
//!
//! The Page Buffer is the reconciliation between the streaming paradigm
//! (cross-page FSM) and the reality that intra-page layout analysis
//! requires full-page context. It guarantees O(1 page) memory usage:
//! primitives for a completed page are released before the next page
//! begins accumulating.
//!
//! # Page boundary detection
//!
//! A page boundary is detected when a primitive arrives with a different
//! `page` number than the current page. The buffer then flushes all
//! accumulated primitives for the completed page and begins accumulating
//! for the new page.
//!
//! # Usage
//!
//! Feed primitives via `ingest()`. Each time a page boundary is crossed,
//! the completed page is returned as a `PageFlush`. After all primitives
//! are ingested, call `finish()` to flush the final page.

use log::warn;

use crate::error::{Warning, WarningKind};
use crate::model::{PageDimensions, Primitive, PrimitiveKind};

use super::types::{FontHistogram, PageFlush, StructureConfig};

// ---------------------------------------------------------------------------
// PageBuffer
// ---------------------------------------------------------------------------

/// Collects primitives page-by-page and flushes completed pages.
///
/// **Memory contract:** After [`flush`] returns, the buffer holds zero
/// primitives from the completed page. Capacity may be retained for reuse.
/// Peak memory is O(max primitives per page), never O(document).
#[derive(Debug)]
pub struct PageBuffer {
    /// The page number currently being accumulated.
    current_page: u32,
    /// Primitives accumulated for the current page.
    primitives: Vec<Primitive>,
    /// Page dimensions for the current page (set from the first primitive
    /// or from external metadata).
    page_dims: Option<PageDimensions>,
    /// Cumulative font histogram across all pages (for heuristic detectors).
    font_histogram: FontHistogram,
    /// Whether any primitive has been ingested yet.
    started: bool,
    /// Warnings generated during buffering.
    warnings: Vec<Warning>,
    /// Configuration.
    config: StructureConfig,
    // -- Monitoring counters --
    /// Total number of pages flushed so far.
    pages_flushed: u32,
    /// Total number of primitives ingested across all pages.
    total_primitives: u64,
    /// Peak number of primitives observed on a single page.
    peak_page_primitives: usize,
}

impl PageBuffer {
    /// Create a new Page Buffer with the given configuration.
    pub fn new(config: StructureConfig) -> Self {
        Self {
            current_page: 0,
            primitives: Vec::new(),
            page_dims: None,
            font_histogram: FontHistogram::default(),
            started: false,
            warnings: Vec::new(),
            config,
            pages_flushed: 0,
            total_primitives: 0,
            peak_page_primitives: 0,
        }
    }

    /// Create a new Page Buffer with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(StructureConfig::default())
    }

    /// Ingest a primitive. If it belongs to a new page, the previous page
    /// is flushed first and returned as a [`PageFlush`].
    ///
    /// Returns `Some(PageFlush)` when a page boundary is crossed,
    /// `None` otherwise.
    pub fn ingest(&mut self, primitive: Primitive) -> Option<PageFlush> {
        let prim_page = primitive.page;

        // First primitive ever — initialize current page.
        if !self.started {
            self.started = true;
            self.current_page = prim_page;
            self.push(primitive);
            return None;
        }

        // Same page — just accumulate.
        if prim_page == self.current_page {
            self.push(primitive);
            return None;
        }

        // Page boundary detected — flush the completed page.
        if prim_page < self.current_page {
            warn!(
                "PageBuffer: out-of-order page {} after page {} — buffering to current page",
                prim_page, self.current_page
            );
            self.warnings.push(Warning {
                kind: WarningKind::UnexpectedStructure,
                message: format!(
                    "Out-of-order page number: got page {} while buffering page {}",
                    prim_page, self.current_page
                ),
                page: Some(prim_page),
            });
            self.push(primitive);
            return None;
        }

        // prim_page > self.current_page — flush and start new page.
        let flush = self.flush_current();
        self.current_page = prim_page;
        self.push(primitive);
        Some(flush)
    }

    /// Flush the final page. Call this after all primitives have been ingested.
    ///
    /// Returns `Some(PageFlush)` if there are buffered primitives,
    /// `None` if the buffer is empty (no primitives were ever ingested).
    ///
    /// This method consumes `self` to enforce the contract that no further
    /// primitives can be ingested after finishing.
    pub fn finish(mut self) -> Option<PageFlush> {
        if !self.started || self.primitives.is_empty() {
            return None;
        }
        Some(self.flush_current())
    }

    /// Set page dimensions for the current page.
    ///
    /// Call this before ingesting primitives for the page, or at any point
    /// during accumulation. Typically sourced from the decoder's metadata
    /// extraction.
    pub fn set_page_dims(&mut self, dims: PageDimensions) {
        self.page_dims = Some(dims);
    }

    /// Returns a reference to the cumulative font histogram.
    ///
    /// This histogram is updated incrementally as each page flushes and
    /// is available for downstream heuristic detectors.
    pub fn font_histogram(&self) -> &FontHistogram {
        &self.font_histogram
    }

    /// Returns all warnings generated during buffering.
    pub fn warnings(&self) -> &[Warning] {
        &self.warnings
    }

    /// Take ownership of accumulated warnings, leaving the internal vec empty.
    pub fn take_warnings(&mut self) -> Vec<Warning> {
        std::mem::take(&mut self.warnings)
    }

    /// Returns the page number currently being accumulated.
    pub fn current_page(&self) -> u32 {
        self.current_page
    }

    /// Returns the number of primitives currently buffered.
    pub fn buffered_count(&self) -> usize {
        self.primitives.len()
    }

    /// Returns the total number of pages flushed so far.
    ///
    /// This counter is incremented each time a page boundary is crossed
    /// (via [`ingest`](Self::ingest)) or when [`finish`](Self::finish)
    /// flushes the final page.
    pub fn pages_flushed(&self) -> u32 {
        self.pages_flushed
    }

    /// Returns the total number of primitives ingested across all pages.
    pub fn total_primitives(&self) -> u64 {
        self.total_primitives
    }

    /// Returns the peak number of primitives observed on any single page.
    pub fn peak_page_primitives(&self) -> usize {
        self.peak_page_primitives
    }

    // -----------------------------------------------------------------------
    // Internal
    // -----------------------------------------------------------------------

    /// Push a primitive into the current page buffer.
    fn push(&mut self, primitive: Primitive) {
        // Memory guard: warn (but don't crash) if a page has too many primitives.
        if self.primitives.len() == self.config.max_primitives_per_page {
            warn!(
                "PageBuffer: page {} exceeds max_primitives_per_page ({})",
                self.current_page, self.config.max_primitives_per_page
            );
            self.warnings.push(Warning {
                kind: WarningKind::TruncatedContent,
                message: format!(
                    "Page {} exceeds max_primitives_per_page ({})",
                    self.current_page, self.config.max_primitives_per_page
                ),
                page: Some(self.current_page),
            });
        }

        // Record text font sizes in the histogram.
        if let PrimitiveKind::Text { font_size, .. } = &primitive.kind {
            self.font_histogram.record(*font_size);
        }

        self.total_primitives += 1;
        self.primitives.push(primitive);
    }

    /// Flush the current page: drain primitives, update histogram, return PageFlush.
    fn flush_current(&mut self) -> PageFlush {
        // Update monitoring counters.
        self.pages_flushed += 1;
        self.peak_page_primitives = self.peak_page_primitives.max(self.primitives.len());

        // Pre-allocate the next page's Vec based on this page's count.
        // Pages in a document tend to have similar primitive density, so
        // reusing the observed count avoids repeated reallocation.
        let capacity_hint = self.primitives.len();
        let primitives = std::mem::replace(&mut self.primitives, Vec::with_capacity(capacity_hint));
        let page_dims = self.page_dims.take();
        let page = self.current_page;

        PageFlush {
            page,
            primitives,
            page_dims,
            visual_lines: None,
        }
    }
}

#[cfg(test)]
#[path = "buffer/tests.rs"]
mod tests;
