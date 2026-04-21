//! Heuristic table detector — 5-step pipeline for untagged PDFs.
//!
//! The detector supports lattice, partial-grid, and stream strategies, all
//! converging through the same cell construction pipeline. This file acts
//! as the public façade and keeps configuration and trait integration
//! local, while the heavier algorithmic paths live in dedicated sub-modules.
//!
//! # Scope and prior art
//!
//! This detector only runs on untagged PDFs; DOCX / XLSX / HTML tables
//! come from the formats' own structural markup and bypass this pipeline
//! entirely. The 5-step shape and parameter names
//! (`snap_tolerance`, `join_tolerance`, `intersection_tolerance`,
//! `min_edge_length`) are informed by pdfplumber's `table.py` (MIT);
//! see ATTRIBUTION.md. Olga's implementation adds virtual-edge
//! generation for partial grids, cross-page continuation, header-row
//! detection, rowspan / colspan inference, and several algorithmic
//! speed-ups (spatial-hash deduplication, binary-search intersection
//! candidates).

pub mod cells;
pub mod edges;
pub mod intersections;

mod continuation;
mod lattice;
mod stream;

use crate::model::{HintKind, Primitive, PrimitiveKind, SemanticHint};

use super::{DetectedHint, DetectionContext, StructureDetector};

use edges::EdgeConfig;

/// Detects table regions using the 5-step lattice pipeline (bordered
/// tables) with a text-alignment stream-mode fallback.
///
/// Confidence:
/// - 0.85 for lattice tables (explicit grid lines)
/// - 0.65 for stream tables (text alignment only)
pub struct TableDetector {
    /// Confidence for lattice tables (with explicit grid lines).
    pub lattice_confidence: f32,
    /// Confidence for stream tables (text alignment only).
    pub stream_confidence: f32,
    /// Minimum number of columns to consider a region a table.
    pub min_columns: usize,
    /// Minimum number of rows in a candidate table.
    pub min_rows: usize,
    /// Edge processing tolerances for the lattice pipeline.
    pub edge_config: EdgeConfig,
    /// Intersection tolerance: how close edge crossings must be to count.
    pub intersection_tolerance: f32,
    /// X-alignment tolerance for stream mode.
    pub x_tolerance: f32,
    /// Y-gap factor for stream mode.
    pub y_gap_factor: f32,
}

impl Default for TableDetector {
    fn default() -> Self {
        Self {
            lattice_confidence: 0.85,
            stream_confidence: 0.65,
            min_columns: 2,
            min_rows: 2,
            edge_config: EdgeConfig::default(),
            intersection_tolerance: 0.005,
            x_tolerance: 0.015,
            y_gap_factor: 2.0,
        }
    }
}

impl TableDetector {
    /// Validate configuration values and clamp to sensible ranges.
    pub fn validate(&mut self) -> Vec<String> {
        let mut issues = Vec::new();

        if self.lattice_confidence < 0.0 || self.lattice_confidence > 1.0 {
            issues.push(format!(
                "TableDetector: lattice_confidence {} clamped to 0.85",
                self.lattice_confidence
            ));
            self.lattice_confidence = 0.85;
        }
        if self.stream_confidence < 0.0 || self.stream_confidence > 1.0 {
            issues.push(format!(
                "TableDetector: stream_confidence {} clamped to 0.65",
                self.stream_confidence
            ));
            self.stream_confidence = 0.65;
        }
        if self.min_columns < 2 {
            issues.push(format!(
                "TableDetector: min_columns {} clamped to 2",
                self.min_columns
            ));
            self.min_columns = 2;
        }
        if self.min_rows < 2 {
            issues.push(format!(
                "TableDetector: min_rows {} clamped to 2",
                self.min_rows
            ));
            self.min_rows = 2;
        }
        if self.intersection_tolerance <= 0.0 {
            issues.push(format!(
                "TableDetector: intersection_tolerance {} clamped to 0.005",
                self.intersection_tolerance
            ));
            self.intersection_tolerance = 0.005;
        }
        if self.x_tolerance <= 0.0 || self.x_tolerance > 0.2 {
            issues.push(format!(
                "TableDetector: x_tolerance {} clamped to 0.015",
                self.x_tolerance
            ));
            self.x_tolerance = 0.015;
        }
        if self.y_gap_factor <= 1.0 {
            issues.push(format!(
                "TableDetector: y_gap_factor {} clamped to 2.0 (must be > 1.0)",
                self.y_gap_factor
            ));
            self.y_gap_factor = 2.0;
        }

        let ec = &mut self.edge_config;
        if ec.snap_tolerance <= 0.0 {
            issues.push(format!(
                "EdgeConfig: snap_tolerance {} clamped to 0.005",
                ec.snap_tolerance
            ));
            ec.snap_tolerance = 0.005;
        }
        if ec.join_tolerance <= 0.0 {
            issues.push(format!(
                "EdgeConfig: join_tolerance {} clamped to 0.005",
                ec.join_tolerance
            ));
            ec.join_tolerance = 0.005;
        }
        if ec.min_edge_length <= 0.0 {
            issues.push(format!(
                "EdgeConfig: min_edge_length {} clamped to 0.01",
                ec.min_edge_length
            ));
            ec.min_edge_length = 0.01;
        }
        if ec.max_line_thickness <= 0.0 {
            issues.push(format!(
                "EdgeConfig: max_line_thickness {} clamped to 0.008",
                ec.max_line_thickness
            ));
            ec.max_line_thickness = 0.008;
        }

        issues
    }
}

impl StructureDetector for TableDetector {
    fn name(&self) -> &str {
        "table"
    }

    fn detect(&self, page: &[Primitive], context: &DetectionContext) -> Vec<DetectedHint> {
        let lattice_hints = self.detect_lattice(page);
        let hints = if !lattice_hints.is_empty() {
            lattice_hints
        } else {
            self.detect_stream(page, context)
        };

        if let Some(ref prev_columns) = context.open_table_columns {
            let page_columns = Self::extract_column_positions(&hints);
            if Self::columns_match(prev_columns, &page_columns, self.x_tolerance) {
                return Self::promote_to_continuation(hints);
            }
        }

        hints
    }
}

#[cfg(test)]
mod tests;
