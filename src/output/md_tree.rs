//! Structure-based Markdown renderer.
//!
//! Converts a [`StructureResult`] into Markdown by walking the `DocumentNode`
//! tree.  Used for non-PDF formats (DOCX, HTML, XLSX) where the spatial
//! Markdown renderer (`markdown.rs`) cannot operate.
//!
//! Produces standard CommonMark with GFM table extensions.

use crate::model::{DocumentNode, NodeKind};
use crate::structure::StructureResult;

/// Render a [`StructureResult`] as Markdown.
pub fn render(result: &StructureResult) -> String {
    let mut out = String::new();
    let mut current_page: u32 = 0;
    for child in &result.root.children {
        let node_page = child.source_pages.start;
        if node_page > current_page {
            // Insert page separator(s).
            for p in current_page..node_page {
                ensure_blank_line(&mut out);
                out.push_str(&format!("---\n_page {}_\n", p + 2));
            }
            out.push('\n');
            current_page = node_page;
        }
        render_node(child, &mut out, 0);
        // Track the end of the node's page range.
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

        NodeKind::Heading { level, text } => {
            ensure_blank_line(out);
            let hashes = "#".repeat((*level).min(6) as usize);
            out.push_str(&hashes);
            out.push(' ');
            out.push_str(text.trim());
            out.push('\n');
            ensure_blank_line(out);
        }

        NodeKind::Paragraph { text } => {
            ensure_blank_line(out);
            out.push_str(text.trim());
            out.push('\n');
        }

        NodeKind::List { ordered } => {
            ensure_blank_line(out);
            let mut index = 1u32;
            for child in &node.children {
                if let NodeKind::ListItem { text } = &child.kind {
                    let indent = "  ".repeat(depth);
                    if *ordered {
                        out.push_str(&indent);
                        out.push_str(&format!("{}. ", index));
                    } else {
                        out.push_str(&indent);
                        out.push_str("- ");
                    }
                    out.push_str(text.trim());
                    out.push('\n');
                    index += 1;
                }
            }
        }

        NodeKind::ListItem { text } => {
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

        NodeKind::TableRow | NodeKind::TableCell { .. } => {}

        NodeKind::CodeBlock { language, text } => {
            ensure_blank_line(out);
            out.push_str("```");
            if let Some(lang) = language {
                out.push_str(lang);
            }
            out.push('\n');
            out.push_str(text.trim());
            out.push('\n');
            out.push_str("```\n");
        }

        NodeKind::BlockQuote { text } => {
            ensure_blank_line(out);
            for line in text.lines() {
                out.push_str("> ");
                out.push_str(line);
                out.push('\n');
            }
        }

        NodeKind::PageHeader { .. } | NodeKind::PageFooter { .. } => {
            // Skip in markdown — repeated noise.
        }

        NodeKind::Footnote { id, text } => {
            out.push_str("[^");
            out.push_str(id);
            out.push_str("]: ");
            out.push_str(text.trim());
            out.push('\n');
        }

        NodeKind::Image { alt_text, .. } => {
            let alt = alt_text.as_deref().unwrap_or("image");
            out.push_str("![");
            out.push_str(alt);
            out.push_str("]()\n");
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
                // Apply formatting.
                let text = &span.text;
                let formatted = match (span.is_bold, span.is_italic, &span.link_url) {
                    (_, _, Some(url)) => format!("[{}]({})", text, url),
                    (true, true, None) => format!("***{}***", text),
                    (true, false, None) => format!("**{}**", text),
                    (false, true, None) => format!("*{}*", text),
                    (false, false, None) => text.clone(),
                };
                line.push_str(&formatted);
                cursor = span.col_start + span.text.chars().count();
            }
            out.push_str(line.trim_end());
            out.push('\n');
        }
    }
}

/// Render a table as a GFM-style Markdown table.
fn render_table(table_node: &DocumentNode, _rows: usize, cols: usize, out: &mut String) {
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

    // Column widths (minimum 3 for the --- separator).
    let mut widths = vec![0usize; cols];
    for row in &grid {
        for (i, cell) in row.iter().enumerate() {
            if i < cols {
                widths[i] = widths[i].max(cell.len());
            }
        }
    }
    for w in &mut widths {
        if *w < 3 {
            *w = 3;
        }
    }

    // GFM table: header row, separator, data rows.
    for (i, row) in grid.iter().enumerate() {
        out.push_str("| ");
        for (j, cell) in row.iter().enumerate() {
            if j > 0 {
                out.push_str(" | ");
            }
            let w = widths.get(j).copied().unwrap_or(3);
            out.push_str(&format!("{:<width$}", cell, width = w));
        }
        out.push_str(" |\n");

        // GFM separator after the first row (header).
        if i == 0 {
            out.push_str("| ");
            for (j, w) in widths.iter().enumerate() {
                if j > 0 {
                    out.push_str(" | ");
                }
                out.push_str(&"-".repeat(*w));
            }
            out.push_str(" |\n");
        }
    }
}

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
    fn renders_heading_with_hashes() {
        let result = make_result(vec![make_node(NodeKind::Heading {
            level: 2,
            text: "Section".into(),
        })]);
        let md = render(&result);
        assert!(md.contains("## Section"));
    }

    #[test]
    fn renders_gfm_table() {
        let mut table = make_node(NodeKind::Table { rows: 2, cols: 2 });
        let mut row0 = make_node(NodeKind::TableRow);
        row0.children = vec![
            make_node(NodeKind::TableCell {
                row: 0,
                col: 0,
                rowspan: 1,
                colspan: 1,
                text: "A".into(),
            }),
            make_node(NodeKind::TableCell {
                row: 0,
                col: 1,
                rowspan: 1,
                colspan: 1,
                text: "B".into(),
            }),
        ];
        let mut row1 = make_node(NodeKind::TableRow);
        row1.children = vec![
            make_node(NodeKind::TableCell {
                row: 1,
                col: 0,
                rowspan: 1,
                colspan: 1,
                text: "1".into(),
            }),
            make_node(NodeKind::TableCell {
                row: 1,
                col: 1,
                rowspan: 1,
                colspan: 1,
                text: "2".into(),
            }),
        ];
        table.children = vec![row0, row1];

        let result = make_result(vec![table]);
        let md = render(&result);
        assert!(md.contains("| A"));
        assert!(md.contains("| ---"));
        assert!(md.contains("| 1"));
    }

    #[test]
    fn renders_ordered_list() {
        let mut list = make_node(NodeKind::List { ordered: true });
        list.children = vec![
            make_node(NodeKind::ListItem {
                text: "First".into(),
            }),
            make_node(NodeKind::ListItem {
                text: "Second".into(),
            }),
        ];
        let result = make_result(vec![list]);
        let md = render(&result);
        assert!(md.contains("1. First"));
        assert!(md.contains("2. Second"));
    }
}
