use crate::error::Warning;
use crate::model::Primitive;
use crate::traits::DecodeResult;

use crate::structure::assembler::Assembler;
use crate::structure::buffer::PageBuffer;
use crate::structure::detectors::{DetectionContext, apply_detectors};
use crate::structure::reading_order::select_resolver;
use crate::structure::types::{FontHistogram, PageFlush};
use crate::structure::visual_lines::{PageMetrics, group_visual_lines, looks_like_table};

use super::helpers::{looks_like_multicolumn, strip_stream_table_hints, tag_page_numbers};
use super::{StructureEngine, StructureResult};

impl StructureEngine {
    /// Process a [`DecodeResult`] into a structured document tree.
    ///
    /// This is the main entry point. It runs the full four-stage pipeline:
    /// buffering, reading-order correction, heuristic detection, and assembly.
    ///
    /// # Performance
    ///
    /// Memory is bounded to O(max primitives per page). The assembler streams
    /// page-by-page — the entire document is never held in memory at once.
    pub fn structure(&self, decode_result: DecodeResult) -> StructureResult {
        let mut buffer = PageBuffer::new(self.config.clone());
        let mut assembler = Assembler::new(self.config.clone());
        let mut all_warnings: Vec<Warning> = decode_result.warnings;
        let mut heuristic_pages: Vec<u32> = Vec::new();

        for primitive in decode_result.primitives {
            if let Some(flush) = buffer.ingest(primitive) {
                self.process_flushed_page(flush, &buffer, &mut assembler, &mut heuristic_pages);
            }
        }

        let final_font_histogram = buffer.font_histogram().clone();
        let buffer_warnings = buffer.take_warnings();

        if let Some(flush) = buffer.finish() {
            self.process_final_page(
                flush,
                &final_font_histogram,
                &mut assembler,
                &mut heuristic_pages,
            );
        }

        all_warnings.extend(buffer_warnings);

        let assembly = assembler.finish();
        all_warnings.extend(assembly.warnings);

        StructureResult {
            metadata: decode_result.metadata,
            root: assembly.root,
            artifacts: assembly.artifacts,
            warnings: all_warnings,
            heuristic_pages,
        }
    }

    fn process_flushed_page(
        &self,
        mut flush: PageFlush,
        buffer: &PageBuffer,
        assembler: &mut Assembler,
        heuristic_pages: &mut Vec<u32>,
    ) {
        let resolver = select_resolver(
            &flush.primitives,
            self.config.hint_trust_threshold,
            self.config.column_gap_threshold,
        );
        resolver.reorder(&mut flush.primitives);

        let text_boxes = self.run_layout_analysis(&flush.primitives);

        let needs_detection = self.page_needs_detection(&flush.primitives);
        if !self.detectors.is_empty() && needs_detection {
            let context = DetectionContext {
                font_histogram: buffer.font_histogram().clone(),
                page: flush.page,
                page_dims: flush.page_dims,
                text_boxes,
                open_table_columns: assembler.open_table_column_positions(),
            };
            apply_detectors(&mut flush.primitives, &self.detectors, &context);
            heuristic_pages.push(flush.page);
        }

        if needs_detection {
            let metrics = PageMetrics::from_primitives(&flush.primitives);
            let lines = group_visual_lines(&flush.primitives, &metrics);
            if !looks_like_table(&lines, 3, 0.02) {
                strip_stream_table_hints(&mut flush.primitives);
            }
            flush.visual_lines = Some(lines);
            tag_page_numbers(&mut flush.primitives);
        }

        if flush.page == 0
            && let Some(ref lines) = flush.visual_lines
            && looks_like_multicolumn(lines)
        {
            flush.visual_lines = None;
        }

        assembler.process_page(&flush);
    }

    fn process_final_page(
        &self,
        mut flush: PageFlush,
        font_histogram: &FontHistogram,
        assembler: &mut Assembler,
        heuristic_pages: &mut Vec<u32>,
    ) {
        let resolver = select_resolver(
            &flush.primitives,
            self.config.hint_trust_threshold,
            self.config.column_gap_threshold,
        );
        resolver.reorder(&mut flush.primitives);

        let text_boxes = self.run_layout_analysis(&flush.primitives);

        let needs_detection = self.page_needs_detection(&flush.primitives);
        if !self.detectors.is_empty() && needs_detection {
            let context = DetectionContext {
                font_histogram: font_histogram.clone(),
                page: flush.page,
                page_dims: flush.page_dims,
                text_boxes,
                open_table_columns: assembler.open_table_column_positions(),
            };
            apply_detectors(&mut flush.primitives, &self.detectors, &context);
            heuristic_pages.push(flush.page);
        }

        if needs_detection {
            let metrics = PageMetrics::from_primitives(&flush.primitives);
            let lines = group_visual_lines(&flush.primitives, &metrics);
            if !looks_like_table(&lines, 3, 0.02) {
                strip_stream_table_hints(&mut flush.primitives);
            }
            flush.visual_lines = Some(lines);
            tag_page_numbers(&mut flush.primitives);
        }

        if flush.page == 0 {
            flush.visual_lines = None;
        }

        assembler.process_page(&flush);
    }

    fn run_layout_analysis(
        &self,
        primitives: &[Primitive],
    ) -> Option<Vec<crate::structure::layout::TextBox>> {
        let analyzer = self.layout_analyzer.as_ref()?;
        let boxes = analyzer.analyze(primitives);
        if boxes.is_empty() { None } else { Some(boxes) }
    }

    fn page_needs_detection(&self, primitives: &[Primitive]) -> bool {
        if primitives.is_empty() {
            return false;
        }
        let hinted = primitives.iter().filter(|p| !p.hints.is_empty()).count();
        let coverage = hinted as f32 / primitives.len() as f32;
        coverage < self.config.hint_trust_threshold
    }
}
