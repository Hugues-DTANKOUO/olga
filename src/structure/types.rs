//! Shared types for the Structure Engine.
//!
//! These types are used across the Page Buffer, Reading Order Resolver,
//! and Assembler components.

use serde::{Deserialize, Serialize};

use super::visual_lines::VisualLine;
use crate::model::{PageDimensions, Primitive};

// ---------------------------------------------------------------------------
// StructureConfig
// ---------------------------------------------------------------------------

/// Configuration for the Structure Engine pipeline.
///
/// Controls thresholds and toggles for the Page Buffer, Reading Order,
/// and Assembler stages.
#[derive(Debug, Clone)]
pub struct StructureConfig {
    /// Minimum fraction of primitives carrying hints (0.0–1.0) before
    /// heuristic detection is skipped on a page. Default: 0.8.
    pub hint_trust_threshold: f32,

    /// Minimum normalized horizontal gap width to split columns in
    /// XY-Cut reading order. Default: 0.03 (3% of page width).
    pub column_gap_threshold: f32,

    /// Whether to strip `PageHeader`/`PageFooter` primitives from the
    /// content tree. When `true`, they are collected as metadata instead
    /// of appearing in the output tree. Default: true.
    pub strip_artifacts: bool,

    /// Maximum number of primitives per page before a warning is emitted.
    /// Acts as a memory safety guard — the engine does NOT crash, but logs
    /// a warning for pages exceeding this threshold. Default: 50_000.
    pub max_primitives_per_page: usize,
}

impl Default for StructureConfig {
    fn default() -> Self {
        Self {
            hint_trust_threshold: 0.8,
            column_gap_threshold: 0.03,
            strip_artifacts: true,
            max_primitives_per_page: 50_000,
        }
    }
}

// ---------------------------------------------------------------------------
// PageFlush
// ---------------------------------------------------------------------------

/// A completed page ready for reading-order correction and assembly.
///
/// Produced by [`PageBuffer::flush`](super::buffer::PageBuffer) when a page
/// boundary is detected or when the document ends.
#[derive(Debug, Clone)]
pub struct PageFlush {
    /// Zero-indexed page number.
    pub page: u32,

    /// All primitives for this page, in source order.
    /// The Reading Order Resolver will reorder these before the Assembler
    /// consumes them.
    pub primitives: Vec<Primitive>,

    /// Page dimensions (when available). PDF provides these natively;
    /// DOCX/XLSX/HTML may not.
    pub page_dims: Option<PageDimensions>,

    /// Visual lines computed from the page's text primitives.
    ///
    /// Populated by the engine for pages that need heuristic detection.
    /// The assembler uses these to emit `AlignedLine` nodes that preserve
    /// the visual horizontal layout instead of forcing all text into
    /// paragraphs or tables.
    pub visual_lines: Option<Vec<VisualLine>>,
}

// ---------------------------------------------------------------------------
// FontHistogram
// ---------------------------------------------------------------------------

/// Cumulative font-size frequency table built across all flushed pages.
///
/// Used by heuristic detectors (Phase 4) to identify body text vs headings.
/// The histogram is updated incrementally as each page is flushed —
/// no retroactive scan of the entire document.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FontHistogram {
    /// Frequency of each font size (rounded to 1 decimal place).
    /// Key: `(font_size * 10.0) as u32` (avoids float keys).
    entries: std::collections::HashMap<u32, u64>,
    /// Total number of text primitives recorded.
    total: u64,
}

impl FontHistogram {
    /// Record a text primitive's font size.
    pub fn record(&mut self, font_size: f32) {
        let key = (font_size * 10.0).round() as u32;
        *self.entries.entry(key).or_insert(0) += 1;
        self.total += 1;
    }

    /// Record all text primitives in a page flush.
    pub fn record_page(&mut self, primitives: &[Primitive]) {
        for p in primitives {
            if let crate::model::PrimitiveKind::Text { font_size, .. } = &p.kind {
                self.record(*font_size);
            }
        }
    }

    /// Returns the most frequent font size (the likely body text size).
    /// Returns `None` if no text has been recorded.
    pub fn body_font_size(&self) -> Option<f32> {
        self.entries
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(key, _)| *key as f32 / 10.0)
    }

    /// Returns `true` if `font_size` is in the top `percentile` (0.0–1.0)
    /// of recorded font sizes by frequency weight.
    ///
    /// Used by the heading detector: sizes in the top 10% are heading candidates.
    pub fn is_in_top_percentile(&self, font_size: f32, percentile: f32) -> bool {
        if self.total == 0 {
            return false;
        }
        let target_key = (font_size * 10.0).round() as u32;

        // Count how many text primitives use a font size >= the target.
        let count_at_or_above: u64 = self
            .entries
            .iter()
            .filter(|(k, _)| **k >= target_key)
            .map(|(_, v)| *v)
            .sum();

        // The size is in the top percentile if fewer than `percentile` fraction
        // of all text uses this size or larger.
        (count_at_or_above as f64 / self.total as f64) <= percentile as f64
    }

    /// Total number of text primitives recorded.
    pub fn total(&self) -> u64 {
        self.total
    }

    /// Number of distinct font sizes seen.
    pub fn distinct_sizes(&self) -> usize {
        self.entries.len()
    }
}
