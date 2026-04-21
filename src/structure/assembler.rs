//! Assembler — hint-driven FSM that builds a `DocumentNode` tree.
//!
//! The Assembler consumes page-ordered primitives (one [`PageFlush`] at a
//! time) and produces a hierarchical document tree. It is the fast path for
//! DOCX, HTML, and tagged PDF where hints carry the structural information.
//!
//! # Hint-driven assembly rules
//!
//! | Hint | Action |
//! |------|--------|
//! | `SectionStart` | Push section onto stack |
//! | `SectionEnd` | Pop section, emit with children |
//! | `Heading` | Accumulate text, emit as leaf |
//! | `Paragraph` | Accumulate text, emit on kind change |
//! | `TableCell` | Route to open table (create if needed) |
//! | `TableHeader` | Route to open table, mark cell as header |
//! | `ListItem` | Route to open list (create if needed) |
//! | `PageHeader/Footer` | Strip from tree, collect as artifacts |
//! | `CodeBlock/BlockQuote` | Accumulate text, emit as leaf |
//! | `Footnote` | Emit as leaf |
//! | `Sidebar` | Emit as paragraph with `role=sidebar` metadata |
//! | `Image` (bare) | Emit as Image node |
//! | `Line/Rect` (bare) | Skip silently (decoration, not content) |
//! | No hints (text) | Wrap in implicit paragraph |
//!
//! # Inline hint propagation
//!
//! Inline hints (`Link`, `ExpandedText`, `Emphasis`, `Strong`) are
//! propagated as metadata on the emitted `DocumentNode`:
//!
//! - `Link { url }` → `metadata["link_url"]`
//! - `ExpandedText { text }` → `metadata["expanded_text"]`
//! - `Emphasis` / `Strong` → `metadata["emphasis"]` = `"italic"` / `"bold"`
//!
//! # Cross-page continuity (Phase 6)
//!
//! Sections stay open across page boundaries. Tables and lists survive
//! page boundaries when the new page continues with the same structure
//! type (hinted). Paragraphs merge across pages when bare text starts
//! with a lowercase letter or continuation punctuation, **or** when the
//! geometry matches (same X position, similar width, same font size —
//! indicating the same column and paragraph style). Repeated table
//! headers on subsequent pages are detected and skipped.
//!
//! # Span auto-correction
//!
//! When a table is closed, any cell whose `colspan` or `rowspan` exceeds
//! the actual grid dimensions is clamped to fit. A warning is emitted for
//! each clamped span to aid debugging of malformed decoder output.

use crate::error::{Warning, WarningKind};
use crate::model::provenance::StructureSource;
use crate::model::{BoundingBox, DocumentNode, NodeKind, Primitive};

use super::types::{PageFlush, StructureConfig};

use self::types::{OpenList, OpenSection, OpenTable, PrimaryRole, TextAccumulator, TextBlockKind};

mod artifacts;
mod classify;
mod continuity;
mod helpers;
mod lists;
mod sections;
mod tables;
mod text;
mod types;
mod visual;

/// Result of assembling primitives into a document tree.
#[derive(Debug)]
pub struct AssemblyResult {
    /// Root `Document` node containing the entire tree.
    pub root: DocumentNode,
    /// Page artifacts (headers/footers) stripped from the content tree.
    pub artifacts: Vec<DocumentNode>,
    /// Warnings generated during assembly.
    pub warnings: Vec<Warning>,
}

/// Hint-driven FSM that builds a `DocumentNode` tree from page-ordered
/// primitives.
///
/// Feed pages via [`process_page`](Self::process_page), then call
/// [`finish`](Self::finish) to close all open objects and retrieve the
/// completed tree.
#[derive(Debug)]
pub struct Assembler {
    config: StructureConfig,
    output: Vec<DocumentNode>,
    section_stack: Vec<OpenSection>,
    text_acc: Option<TextAccumulator>,
    open_table: Option<OpenTable>,
    open_list: Option<OpenList>,
    artifacts: Vec<DocumentNode>,
    warnings: Vec<Warning>,
    last_page: Option<u32>,
    font_size_counts: std::collections::HashMap<u32, u32>,
}

impl Assembler {
    /// Create a new Assembler with the given configuration.
    pub fn new(config: StructureConfig) -> Self {
        Self {
            config,
            output: Vec::new(),
            section_stack: Vec::new(),
            text_acc: None,
            open_table: None,
            open_list: None,
            artifacts: Vec::new(),
            warnings: Vec::new(),
            last_page: None,
            font_size_counts: std::collections::HashMap::new(),
        }
    }

    /// Create a new Assembler with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(StructureConfig::default())
    }

    /// Process one page of primitives (already in reading order).
    pub fn process_page(&mut self, flush: &PageFlush) {
        let is_new_page = self.last_page.is_some_and(|lp| lp != flush.page);
        self.last_page = Some(flush.page);

        if is_new_page {
            self.on_page_boundary(&flush.primitives, flush.page);
        }

        let vline_index = Self::build_vline_index(flush);
        let mut emitted_lines: std::collections::HashSet<usize> = std::collections::HashSet::new();

        for prim in &flush.primitives {
            self.track_font_size(prim);
        }

        let mut last_single_vline: Option<usize> = None;

        for (prim_idx, prim) in flush.primitives.iter().enumerate() {
            if let Some(&(line_idx, false)) = vline_index.get(&prim_idx) {
                if let Some(prev_line) = last_single_vline
                    && line_idx != prev_line
                {
                    self.flush_text_acc();
                }
                last_single_vline = Some(line_idx);
            } else {
                last_single_vline = None;
            }

            if let Some((line_idx, true)) = vline_index.get(&prim_idx) {
                if emitted_lines.contains(line_idx) {
                    continue;
                }

                if let Some(ref lines) = flush.visual_lines {
                    let vline = &lines[*line_idx];
                    let has_heading = vline.spans.iter().any(|s| {
                        if s.prim_index >= flush.primitives.len() {
                            return false;
                        }
                        let p = &flush.primitives[s.prim_index];
                        if let PrimaryRole::Heading { confidence, .. } = Self::classify(p) {
                            !self.should_demote_heading(p, confidence)
                        } else {
                            false
                        }
                    });

                    if has_heading {
                        emitted_lines.insert(*line_idx);
                        self.emit_merged_heading(vline, &flush.primitives, flush.page);
                        continue;
                    }
                }

                if let Some(ref lines) = flush.visual_lines {
                    let vline = &lines[*line_idx];
                    let has_table = vline.spans.iter().any(|s| {
                        s.prim_index < flush.primitives.len()
                            && matches!(
                                Self::classify(&flush.primitives[s.prim_index]),
                                PrimaryRole::TableCell { .. }
                                    | PrimaryRole::TableCellContinuation { .. }
                                    | PrimaryRole::ContainedInTable { .. }
                            )
                    });

                    if !has_table {
                        emitted_lines.insert(*line_idx);
                        self.emit_aligned_line(vline, &flush.primitives, flush.page);
                        continue;
                    }
                }
            }

            self.process_primitive(prim, flush.page);
        }
    }

    /// Close all open objects and produce the final document tree.
    pub fn finish(mut self) -> AssemblyResult {
        self.flush_text_acc();
        self.close_table();
        self.close_list();

        let unclosed_count = self.section_stack.len();
        if unclosed_count > 0 {
            self.warnings.push(Warning {
                kind: WarningKind::PartialExtraction,
                message: format!(
                    "{} section(s) still open at end of document (auto-closed)",
                    unclosed_count
                ),
                page: None,
            });
        }
        while !self.section_stack.is_empty() {
            self.close_top_section();
        }

        let bbox = self.merged_bbox(&self.output);
        let max_page = self
            .output
            .iter()
            .map(|n| n.source_pages.end)
            .max()
            .unwrap_or(1);

        let mut root = DocumentNode::new(
            NodeKind::Document,
            bbox,
            1.0,
            StructureSource::HintAssembled,
            0..max_page,
        );
        root.children = self.output;

        AssemblyResult {
            root,
            artifacts: self.artifacts,
            warnings: self.warnings,
        }
    }

    fn process_primitive(&mut self, prim: &Primitive, page: u32) {
        self.track_font_size(prim);
        let mut role = Self::classify(prim);

        if let PrimaryRole::Heading { confidence, .. } = &role
            && self.should_demote_heading(prim, *confidence)
        {
            role = PrimaryRole::Paragraph {
                confidence: confidence * 0.8,
            };
        }

        match role {
            PrimaryRole::SectionStart { title } => {
                self.flush_text_acc();
                self.close_table();
                self.close_list();
                let level = self.infer_section_level();
                self.open_section(level, title.clone(), page);
            }
            PrimaryRole::SectionEnd => {
                self.flush_text_acc();
                self.close_table();
                self.close_list();
                if self.section_stack.is_empty() {
                    self.warnings.push(Warning {
                        kind: WarningKind::UnexpectedStructure,
                        message: "SectionEnd without matching SectionStart".to_string(),
                        page: Some(page),
                    });
                } else {
                    self.close_top_section();
                }
            }
            PrimaryRole::Heading { level, confidence } => {
                self.close_table();
                self.close_list();
                self.accumulate_text(prim, page, TextBlockKind::Heading(level), confidence);
            }
            PrimaryRole::Paragraph { confidence } => {
                self.flush_text_acc();
                self.close_table();
                self.close_list();
                self.accumulate_text(prim, page, TextBlockKind::Paragraph, confidence);
            }
            PrimaryRole::TableCell {
                row,
                col,
                rowspan,
                colspan,
                is_header,
                confidence,
            } => {
                self.flush_text_acc();
                self.close_list();
                if row == 0
                    && col == 0
                    && let Some(ref existing) = self.open_table
                {
                    let is_repeated_header = existing.repeated_header_pages.contains(&page);
                    let cell_zero_exists =
                        existing.cells.get(&0).is_some_and(|r| r.contains_key(&0));
                    if cell_zero_exists && !is_repeated_header {
                        self.close_table();
                    }
                }
                self.route_to_table(
                    prim, page, row, col, rowspan, colspan, is_header, confidence,
                );
            }
            PrimaryRole::TableCellContinuation {
                row,
                col,
                rowspan,
                colspan,
                confidence,
            } => {
                self.flush_text_acc();
                self.close_list();
                let base = if let Some(ref mut table) = self.open_table {
                    *table
                        .continuation_row_base
                        .get_or_insert_with(|| table.cells.keys().last().copied().unwrap_or(0) + 1)
                } else {
                    0
                };
                let offset_row = row + base;
                self.route_to_table(
                    prim, page, offset_row, col, rowspan, colspan, false, confidence,
                );
            }
            PrimaryRole::ContainedInTable {
                row,
                col,
                confidence,
            } => {
                if self.open_table.is_some() {
                    self.route_to_table(prim, page, row, col, 1, 1, false, confidence);
                } else {
                    self.accumulate_text(prim, page, TextBlockKind::Paragraph, confidence);
                }
            }
            PrimaryRole::ListItem {
                depth,
                ordered,
                confidence,
                list_group,
            } => {
                self.flush_text_acc();
                self.close_table();
                self.route_to_list(prim, page, depth, ordered, confidence, list_group);
            }
            PrimaryRole::PageHeader { confidence } => {
                self.flush_text_acc();
                self.handle_artifact(prim, page, true, confidence);
            }
            PrimaryRole::PageFooter { confidence } => {
                self.flush_text_acc();
                self.handle_artifact(prim, page, false, confidence);
            }
            PrimaryRole::CodeBlock { confidence } => {
                self.close_table();
                self.close_list();
                self.accumulate_text(prim, page, TextBlockKind::CodeBlock, confidence);
            }
            PrimaryRole::BlockQuote { confidence } => {
                self.close_table();
                self.close_list();
                self.accumulate_text(prim, page, TextBlockKind::BlockQuote, confidence);
            }
            PrimaryRole::Footnote { id, confidence } => {
                self.flush_text_acc();
                let text = Self::text_of(prim);
                let structure_source = Self::structure_source_for(prim);
                let mut node = DocumentNode::new(
                    NodeKind::Footnote {
                        id: id.to_string(),
                        text,
                    },
                    prim.bbox,
                    confidence,
                    structure_source,
                    page..page + 1,
                );
                Self::propagate_inline_metadata(prim, &mut node);
                self.emit_node(node);
            }
            PrimaryRole::DocumentTitle => {
                self.close_table();
                self.close_list();
                self.accumulate_text(prim, page, TextBlockKind::Heading(1), 1.0);
            }
            PrimaryRole::Sidebar { confidence } => {
                self.flush_text_acc();
                self.close_table();
                self.close_list();
                let text = Self::text_of(prim);
                if !text.is_empty() {
                    let structure_source = Self::structure_source_for(prim);
                    let mut node = DocumentNode::new(
                        NodeKind::Paragraph { text },
                        prim.bbox,
                        confidence,
                        structure_source,
                        page..page + 1,
                    );
                    node.metadata
                        .insert("role".to_string(), "sidebar".to_string());
                    Self::propagate_inline_metadata(prim, &mut node);
                    self.emit_node(node);
                }
            }
            PrimaryRole::ImageBare => {
                self.flush_text_acc();
                self.close_table();
                self.close_list();
                self.emit_image_node(prim, page);
            }
            PrimaryRole::DecorationBare => {
                self.flush_text_acc();
            }
            PrimaryRole::TextBare => {
                self.close_table();
                self.close_list();
                self.accumulate_text(prim, page, TextBlockKind::Paragraph, 1.0);
            }
        }
    }
}

#[cfg(test)]
#[path = "assembler/tests.rs"]
mod tests;
