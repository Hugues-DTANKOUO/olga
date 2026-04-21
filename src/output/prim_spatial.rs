//! Sequential renderer for non-PDF formats (DOCX, XLSX).
//!
//! Takes raw [`Primitive`]s from the decoder (with semantic hints) and renders
//! them as clean text or Markdown.  Unlike the PDF spatial renderer which
//! positions characters on a coordinate grid, this renderer works
//! **sequentially**: it iterates primitives in source order and uses semantic
//! hints (headings, bold, tables, lists) to produce structured output.
//!
//! This is the right approach for DOCX/XLSX because their bounding boxes are
//! synthetic (flow-based or grid-indexed), not physical page coordinates.
//!
//! ## Algorithm
//!
//! 1. Group primitives by page (source order preserved)
//! 2. Walk each page's primitives sequentially
//! 3. Detect table regions (contiguous runs of `TableCell` / `TableHeader` hints)
//! 4. Emit non-table text with semantic formatting
//! 5. Emit collected table regions as GFM (markdown) or aligned ASCII (text)

#[path = "prim_spatial/blocks.rs"]
mod blocks;
#[path = "prim_spatial/render.rs"]
mod render;
#[path = "prim_spatial/tables.rs"]
mod tables;
#[cfg(test)]
#[path = "prim_spatial/tests.rs"]
mod tests;

use crate::model::{DocumentMetadata, Primitive};

use self::blocks::extract_blocks;
use self::render::{ensure_blank_line, render_blocks};

/// Render primitives as plain text (sequential, hint-aware).
pub fn render_text(primitives: &[Primitive], metadata: &DocumentMetadata) -> String {
    render_impl(primitives, metadata, false)
}

/// Render primitives as Markdown (sequential, hint-enriched).
pub fn render_markdown(primitives: &[Primitive], metadata: &DocumentMetadata) -> String {
    render_impl(primitives, metadata, true)
}

fn render_impl(primitives: &[Primitive], metadata: &DocumentMetadata, markdown: bool) -> String {
    // `metadata.page_count` is a best-effort estimate. For DOCX in
    // particular it can come from `docProps/app.xml` (`<Pages>`) which is
    // written by the authoring app and frequently under-reports: a file
    // saved by Word on macOS or synthesized programmatically can report
    // `<Pages>1</Pages>` even when the body contains explicit
    // `w:br w:type="page"` breaks that define additional pages. If we
    // iterated only `0..metadata.page_count`, every primitive whose
    // `page` field sits past that ceiling would be silently dropped —
    // and that is exactly what the 2026-04-19 vendor evaluation flagged
    // on memo.docx: the renderer stopped at section 3 even though the
    // decoder had correctly emitted all 6 sections. The rule here is
    // absolute: the renderer must cover every primitive the decoder
    // produced, independent of what the pagination resolver decided.
    //
    // We therefore render the union of (0..metadata.page_count) and the
    // set of page indices actually present on primitives. Pages in that
    // range with zero primitives fall through the `is_empty` guard below
    // without producing any output — i.e. no spurious blank pages.
    let max_prim_page = primitives.iter().map(|p| p.page).max();
    let page_upper_exclusive = match (metadata.page_count, max_prim_page) {
        (0, None) => 0,
        (n, None) => n,
        (n, Some(m)) => n.max(m + 1),
    };
    let mut out = String::new();

    for page_idx in 0..page_upper_exclusive {
        let mut page_prims: Vec<&Primitive> =
            primitives.iter().filter(|p| p.page == page_idx).collect();
        page_prims.sort_by_key(|p| p.source_order);

        if page_prims.is_empty() {
            continue;
        }

        if page_idx > 0 {
            ensure_blank_line(&mut out);
            if markdown {
                out.push_str("---\n");
            } else {
                out.push_str(&format!("--- page {} ---\n", page_idx + 1));
            }
            out.push('\n');
        }

        let blocks = extract_blocks(&page_prims);
        render_blocks(&blocks, &mut out, markdown);
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
