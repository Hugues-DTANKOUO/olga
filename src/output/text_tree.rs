//! Structure-based plain text renderer.
//!
//! Converts a [`StructureResult`] into plain text by walking the `DocumentNode`
//! tree.  Used for non-PDF formats (DOCX, HTML, XLSX) where the spatial
//! renderer (`spatial.rs`) cannot operate because there are no character-level
//! bounding boxes from pdf_oxide.
//!
//! Design philosophy: emit clean, readable plain text that preserves the
//! document's logical structure (headings, paragraphs, lists, tables) without
//! any markup.  Tables are rendered as aligned columns using Unicode box
//! drawing or simple ASCII separators.

use crate::model::{DocumentNode, NodeKind};
use crate::structure::StructureResult;

/// Render a [`StructureResult`] as plain text.
pub fn render(result: &StructureResult) -> String {
    let mut out = String::new();
    let mut current_page: u32 = 0;
    for child in &result.root.children {
        let node_page = child.source_pages.start;
        if node_page > current_page {
            for p in current_page..node_page {
                ensure_blank_line(&mut out);
                out.push_str(&format!("--- page {} ---\n", p + 2));
            }
            out.push('\n');
            current_page = node_page;
        }
        render_node(child, &mut out, 0);
        if child.source_pages.end > current_page + 1 {
            current_page = child.source_pages.end - 1;
        }
    }
    let trimmed = out.trim_end();
    if trimmed.is_empty() {
        String::new()
    } else {
        let mut s = trimmed.to_string();
        s.push('\n');
        s
    }
}

fn render_node(node: &DocumentNode, out: &mut String, depth: usize) {
    match &node.kind {
        NodeKind::Document => {
            for child in &node.children {
                render_node(child, out, depth);
            }
        }

        NodeKind::Section { .. } => {
            for child in &node.children {
                render_node(child, out, depth);
            }
        }

        NodeKind::Heading { text, .. } => {
            ensure_blank_line(out);
            out.push_str(text.trim());
            out.push('\n');
            ensure_blank_line(out);
        }

        NodeKind::Paragraph { text } => {
            ensure_blank_line(out);
            out.push_str(text.trim());
            out.push('\n');
        }

        NodeKind::List { .. } => {
            ensure_blank_line(out);
            let mut index = 1u32;
            for child in &node.children {
                if let NodeKind::ListItem { text } = &child.kind {
                    let prefix = if matches!(node.kind, NodeKind::List { ordered: true }) {
                        format!("{}. ", index)
                    } else {
                        "- ".to_string()
                    };
                    let indent = "  ".repeat(depth);
                    out.push_str(&indent);
                    out.push_str(&prefix);
                    out.push_str(text.trim());
                    out.push('\n');
                    index += 1;
                }
            }
        }

        NodeKind::ListItem { text } => {
            // Standalone list item (shouldn't happen normally, but handle gracefully).
            let indent = "  ".repeat(depth);
            out.push_str(&indent);
            out.push_str("- ");
            out.push_str(text.trim());
            out.push('\n');
        }

        NodeKind::Table { rows, cols } => {
            ensure_blank_line(out);
            render_table(node, *rows as usize, *cols as usize, out);
            out.push('\n');
        }

        NodeKind::TableRow | NodeKind::TableCell { .. } => {
            // These are handled by render_table — skip if encountered at top level.
        }

        NodeKind::CodeBlock { text, .. } => {
            ensure_blank_line(out);
            out.push_str(text.trim());
            out.push('\n');
        }

        NodeKind::BlockQuote { text } => {
            ensure_blank_line(out);
            for line in text.lines() {
                out.push_str("> ");
                out.push_str(line);
                out.push('\n');
            }
        }

        NodeKind::PageHeader { text } | NodeKind::PageFooter { text } => {
            // Skip headers/footers in plain text — they're repeated noise.
            let _ = text;
        }

        NodeKind::Footnote { id, text } => {
            out.push('[');
            out.push_str(id);
            out.push_str("] ");
            out.push_str(text.trim());
            out.push('\n');
        }

        NodeKind::Image { alt_text, .. } => {
            if let Some(alt) = alt_text
                && !alt.is_empty()
            {
                out.push_str("[Image: ");
                out.push_str(alt);
                out.push_str("]\n");
            }
        }

        NodeKind::AlignedLine { spans } => {
            let mut line = String::new();
            let mut cursor: usize = 0;
            for span in spans {
                if span.col_start > cursor {
                    for _ in 0..(span.col_start - cursor) {
                        line.push(' ');
                    }
                }
                line.push_str(&span.text);
                cursor = span.col_start + span.text.chars().count();
            }
            out.push_str(line.trim_end());
            out.push('\n');
        }
    }
}

/// Render a table as aligned ASCII columns.
fn render_table(table_node: &DocumentNode, _rows: usize, cols: usize, out: &mut String) {
    // Collect cell texts into a grid.
    let mut grid: Vec<Vec<String>> = Vec::new();

    for row_node in &table_node.children {
        match &row_node.kind {
            NodeKind::TableRow => {
                let mut row_cells: Vec<(u32, String)> = Vec::new();
                for cell_node in &row_node.children {
                    if let NodeKind::TableCell { col, text, .. } = &cell_node.kind {
                        row_cells.push((*col, text.clone()));
                    }
                }
                row_cells.sort_by_key(|(c, _)| *c);

                let mut row = vec![String::new(); cols];
                for (c, text) in row_cells {
                    if (c as usize) < cols {
                        row[c as usize] = text;
                    }
                }
                grid.push(row);
            }
            NodeKind::TableCell { col, text, .. } => {
                // Direct child cell (no row wrapper) — ensure grid has enough rows.
                if grid.is_empty() {
                    grid.push(vec![String::new(); cols]);
                }
                let last = grid.last_mut().expect("just ensured non-empty");
                if (*col as usize) < cols {
                    last[*col as usize] = text.clone();
                }
            }
            _ => {}
        }
    }

    if grid.is_empty() {
        return;
    }

    // Compute column widths.
    let mut widths = vec![0usize; cols];
    for row in &grid {
        for (i, cell) in row.iter().enumerate() {
            if i < cols {
                widths[i] = widths[i].max(cell.len());
            }
        }
    }
    // Minimum width of 3 for readability.
    for w in &mut widths {
        if *w < 3 {
            *w = 3;
        }
    }

    // Render rows.
    for (i, row) in grid.iter().enumerate() {
        let line: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(j, cell)| {
                if j < widths.len() {
                    format!("{:<width$}", cell, width = widths[j])
                } else {
                    cell.clone()
                }
            })
            .collect();
        out.push_str(&line.join(" | "));
        out.push('\n');

        // Separator after header row.
        if i == 0 {
            let sep: Vec<String> = widths.iter().map(|w| "-".repeat(*w)).collect();
            out.push_str(&sep.join("-+-"));
            out.push('\n');
        }
    }
}

/// Ensure there's a blank line before the next block element.
fn ensure_blank_line(out: &mut String) {
    if out.is_empty() {
        return;
    }
    if !out.ends_with("\n\n") {
        if out.ends_with('\n') {
            out.push('\n');
        } else {
            out.push_str("\n\n");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::provenance::StructureSource;
    use crate::model::{BoundingBox, DocumentFormat, DocumentMetadata, DocumentNode, NodeKind};
    use crate::model::{PaginationBasis, PaginationProvenance};
    use crate::structure::StructureResult;

    fn test_metadata() -> DocumentMetadata {
        DocumentMetadata {
            format: DocumentFormat::Docx,
            page_count: 1,
            page_count_provenance: PaginationProvenance::observed(PaginationBasis::PhysicalPages),
            page_dimensions: vec![],
            encrypted: false,
            text_extraction_allowed: true,
            file_size: 1024,
            title: None,
        }
    }

    fn make_node(kind: NodeKind) -> DocumentNode {
        DocumentNode {
            kind,
            bbox: BoundingBox::new(0.0, 0.0, 1.0, 0.02),
            confidence: 1.0,
            structure_source: StructureSource::HintAssembled,
            source_pages: 0..1,
            children: vec![],
            metadata: Default::default(),
        }
    }

    fn make_result(children: Vec<DocumentNode>) -> StructureResult {
        let mut root = make_node(NodeKind::Document);
        root.children = children;
        StructureResult {
            metadata: test_metadata(),
            root,
            artifacts: vec![],
            warnings: vec![],
            heuristic_pages: vec![],
        }
    }

    #[test]
    fn renders_heading_and_paragraph() {
        let result = make_result(vec![
            make_node(NodeKind::Heading {
                level: 1,
                text: "Title".into(),
            }),
            make_node(NodeKind::Paragraph {
                text: "Hello world.".into(),
            }),
        ]);
        let text = render(&result);
        assert!(text.contains("Title"));
        assert!(text.contains("Hello world."));
    }

    #[test]
    fn renders_list() {
        let mut list = make_node(NodeKind::List { ordered: false });
        list.children = vec![
            make_node(NodeKind::ListItem {
                text: "Alpha".into(),
            }),
            make_node(NodeKind::ListItem {
                text: "Beta".into(),
            }),
        ];
        let result = make_result(vec![list]);
        let text = render(&result);
        assert!(text.contains("- Alpha"));
        assert!(text.contains("- Beta"));
    }

    #[test]
    fn renders_table() {
        let mut table = make_node(NodeKind::Table { rows: 2, cols: 2 });
        let mut row0 = make_node(NodeKind::TableRow);
        row0.children = vec![
            make_node(NodeKind::TableCell {
                row: 0,
                col: 0,
                rowspan: 1,
                colspan: 1,
                text: "Name".into(),
            }),
            make_node(NodeKind::TableCell {
                row: 0,
                col: 1,
                rowspan: 1,
                colspan: 1,
                text: "Age".into(),
            }),
        ];
        let mut row1 = make_node(NodeKind::TableRow);
        row1.children = vec![
            make_node(NodeKind::TableCell {
                row: 1,
                col: 0,
                rowspan: 1,
                colspan: 1,
                text: "Alice".into(),
            }),
            make_node(NodeKind::TableCell {
                row: 1,
                col: 1,
                rowspan: 1,
                colspan: 1,
                text: "30".into(),
            }),
        ];
        table.children = vec![row0, row1];

        let result = make_result(vec![table]);
        let text = render(&result);
        assert!(text.contains("Name"));
        assert!(text.contains("Alice"));
        assert!(text.contains("---")); // separator
    }
}
