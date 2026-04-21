use super::blocks::Block;
use super::tables::{render_table_ascii, render_table_gfm};

pub(super) fn render_blocks(blocks: &[Block], out: &mut String, markdown: bool) {
    let mut i = 0;
    while i < blocks.len() {
        let block = &blocks[i];

        // Detect table regions: contiguous run of blocks with table_cell set
        // that all belong to the same story.
        //
        // A table inside a page footer is a *different* table from a body
        // table above it, even though both carry `TableHeader { col }`
        // hints that map to row 0. Without the story break, the body table
        // and the footer table merge into one grid and the smaller footer
        // overwrites the larger body's first columns (memo.docx: footer's
        // "Confidential" / "Page 1 of 1" overwriting the approval matrix's
        // "Expense category" / "Up to $500"). See `BlockStory` in
        // `blocks.rs` for the industry rationale.
        if block.table_cell.is_some() {
            let table_start = i;
            let table_story = block.story;
            while i < blocks.len()
                && blocks[i].table_cell.is_some()
                && blocks[i].story == table_story
            {
                i += 1;
            }
            let table_blocks = &blocks[table_start..i];
            ensure_blank_line(out);
            if markdown {
                render_table_gfm(table_blocks, out);
            } else {
                render_table_ascii(table_blocks, out);
            }
            continue;
        }

        // Non-table block.
        if markdown {
            render_block_markdown(block, out);
        } else {
            render_block_text(block, out);
        }
        i += 1;
    }
}

/// Render a single block as plain text.
fn render_block_text(block: &Block, out: &mut String) {
    if block.heading_level > 0 {
        // Headings get blank lines around them.
        ensure_blank_line(out);
        out.push_str(&block.text);
        out.push('\n');
    } else if block.is_list_item {
        // List items — prefix with bullet or number.
        if !already_prefixed(&block.text) {
            if block.list_ordered {
                out.push_str("  ");
                // We don't track the actual index, so just use a dash.
                out.push_str("- ");
            } else {
                out.push_str("- ");
            }
        }
        out.push_str(&block.text);
        out.push('\n');
    } else {
        // Regular paragraph.
        ensure_blank_line(out);
        out.push_str(&block.text);
        out.push('\n');
    }
}

/// Render a single block as Markdown.
fn render_block_markdown(block: &Block, out: &mut String) {
    if block.heading_level > 0 {
        ensure_blank_line(out);
        let hashes = "#".repeat(block.heading_level.min(6) as usize);
        out.push_str(&hashes);
        out.push(' ');
        out.push_str(&block.text);
        out.push('\n');
    } else if block.is_list_item {
        if !already_prefixed(&block.text) {
            if block.list_ordered {
                out.push_str("1. ");
            } else {
                out.push_str("- ");
            }
        }
        let formatted = format_inline_markdown(block);
        out.push_str(&formatted);
        out.push('\n');
    } else {
        ensure_blank_line(out);
        let formatted = format_inline_markdown(block);
        out.push_str(&formatted);
        out.push('\n');
    }
}

/// Apply inline Markdown formatting (bold, italic, links).
fn format_inline_markdown(block: &Block) -> String {
    let mut text = block.text.clone();

    // Bold / italic wrapping.
    if block.is_bold && block.is_italic {
        text = format!("***{}***", text);
    } else if block.is_bold {
        text = format!("**{}**", text);
    } else if block.is_italic {
        text = format!("*{}*", text);
    }

    // Link wrapping.
    if let Some(ref url) = block.link_url {
        text = format!("[{}]({})", text, url);
    }

    text
}

/// Check if text already looks like a list item (starts with "- ", "* ", or "1. " etc).
fn already_prefixed(text: &str) -> bool {
    text.starts_with("- ")
        || text.starts_with("* ")
        || text.chars().next().is_some_and(|c| c.is_ascii_digit()) && text.contains(". ")
}
pub(super) fn ensure_blank_line(out: &mut String) {
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
