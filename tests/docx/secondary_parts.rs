//! Regression tests for DOCX secondary-part handling.
//!
//! Secondary parts (header*.xml, footer*.xml, footnotes.xml, endnotes.xml,
//! comments.xml) are page chrome — they are not part of the linear reading
//! flow of `word/document.xml`. Historically olga parsed them at
//! `(page=0, y=0.0)`, which caused two observable defects on real-world
//! documents (memo.docx):
//!
//!   1. **Non-deterministic output.** `doc_rels` is a `HashMap`, so
//!      `doc_rels.values()` iterates in randomized order per process.
//!      Footer primitives picked up different `source_order` values on each
//!      run, and the markdown renderer — which sorts by `source_order` —
//!      sometimes interleaved footer rows with body tables.
//!
//!   2. **Body-table collision.** When the footer contained a table, its
//!      `TableHeader { col }` hints (row = 0 by convention) landed next to
//!      the body table's row-0 hints; the block assembler then merged them
//!      into a single markdown table, and footer columns overwrote body
//!      columns (most visibly "Expense category" / "Up to $500" on memo's
//!      approval matrix).
//!
//! The fix in `process_secondary_parts` applies the secondary-story
//! ordering convention used by mainstream DOCX extractors (Apache POI's
//! `XWPFWordExtractor`, python-docx, Mammoth): headers → body → footers
//! → footnotes → endnotes → comments — and pins every secondary primitive to
//! the last body page past the lowest body primitive. These tests lock in
//! both properties.

use olga::formats::docx::DocxDecoder;
use olga::model::*;
use olga::traits::FormatDecoder;

use crate::support::docx::build_docx_with_parts_and_rels;

/// Build a DOCX whose body contains a 3-column table and whose footer
/// contains a 2-column table. This reproduces the memo.docx topology that
/// used to show footer columns overwriting body approval-matrix headers.
fn build_docx_with_body_table_and_footer_table() -> Vec<u8> {
    let body = r#"
    <w:p><w:r><w:t>Intro paragraph</w:t></w:r></w:p>
    <w:tbl>
      <w:tblPr><w:tblStyle w:val="TableGrid"/></w:tblPr>
      <w:tr>
        <w:tc><w:p><w:r><w:t>Expense category</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>Up to $500</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>Above $500</w:t></w:r></w:p></w:tc>
      </w:tr>
      <w:tr>
        <w:tc><w:p><w:r><w:t>Travel</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>Manager</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>Director</w:t></w:r></w:p></w:tc>
      </w:tr>
    </w:tbl>
    "#;

    let footer_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:tbl>
    <w:tr>
      <w:tc><w:p><w:r><w:t>Confidential</w:t></w:r></w:p></w:tc>
      <w:tc><w:p><w:r><w:t>Page 1 of 1</w:t></w:r></w:p></w:tc>
    </w:tr>
  </w:tbl>
</w:ftr>"#;

    let rels = [
        r#"<Relationship Id="rIdFooter1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/>"#,
    ];

    build_docx_with_parts_and_rels(body, None, None, &rels, &[("word/footer1.xml", footer_xml)])
}

#[test]
fn footer_primitives_are_segregated_from_body_on_last_page() {
    // Under the industry-standard segregation policy (Apache POI-style),
    // footer primitives must land on the last body page with `y` strictly
    // greater than every body primitive on that page. This guarantees the
    // markdown renderer and block assembler can never merge a footer row
    // with a body table row — regardless of HashMap iteration order.
    let docx = build_docx_with_body_table_and_footer_table();
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    let body_cells: Vec<&Primitive> = result
        .primitives
        .iter()
        .filter(|p| {
            matches!(&p.kind, PrimitiveKind::Text { content, .. }
                if content == "Expense category"
                    || content == "Up to $500"
                    || content == "Above $500"
                    || content == "Travel"
                    || content == "Manager"
                    || content == "Director"
                    || content == "Intro paragraph")
        })
        .collect();
    assert_eq!(
        body_cells.len(),
        7,
        "body cells must all be preserved (1 intro + 6 table cells)"
    );

    let footer_cells: Vec<&Primitive> = result
        .primitives
        .iter()
        .filter(|p| {
            p.hints
                .iter()
                .any(|h| matches!(h.kind, HintKind::PageFooter))
        })
        .collect();
    assert_eq!(
        footer_cells.len(),
        2,
        "footer must preserve both cells (POI-style preserve-but-segregate)"
    );

    // Every footer primitive lives on a body page (not at page=0 unless
    // the body is also there) and sits past the lowest body primitive.
    let last_body_page = body_cells.iter().map(|p| p.page).max().unwrap();
    let body_max_y_on_last_page = body_cells
        .iter()
        .filter(|p| p.page == last_body_page)
        .map(|p| p.bbox.y + p.bbox.height)
        .fold(0.0f32, f32::max);

    for footer_prim in &footer_cells {
        assert_eq!(
            footer_prim.page, last_body_page,
            "footer primitives must be pinned to the last body page"
        );
        assert!(
            footer_prim.bbox.y > body_max_y_on_last_page,
            "footer primitive at y={} must sit past body max y={} on page {}",
            footer_prim.bbox.y,
            body_max_y_on_last_page,
            last_body_page
        );
    }
}

#[test]
fn footer_table_does_not_collide_with_body_table_in_markdown() {
    // Regression guard: before the fix, both the body's 3-column table
    // header ("Expense category" / "Up to $500" / "Above $500") and the
    // footer's 2-column table ("Confidential" / "Page 1 of 1") carried
    // `TableHeader { col }` hints with row=0. The markdown block assembler
    // groups contiguous table-cell primitives into one markdown table, so
    // the footer used to overwrite the first two body columns. We verify
    // the body header cells reach the markdown output intact.
    let docx = build_docx_with_body_table_and_footer_table();
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();
    let markdown =
        olga::output::prim_spatial::render_markdown(&result.primitives, &result.metadata);

    assert!(
        markdown.contains("Expense category"),
        "body header 'Expense category' must survive: {}",
        markdown
    );
    assert!(
        markdown.contains("Up to $500"),
        "body header 'Up to $500' must survive: {}",
        markdown
    );
    assert!(
        markdown.contains("Above $500"),
        "body header 'Above $500' must survive: {}",
        markdown
    );
    assert!(
        markdown.contains("Travel"),
        "body data cell 'Travel' must survive: {}",
        markdown
    );
}

#[test]
fn secondary_parts_output_is_deterministic_across_decodes() {
    // `doc_rels` is a `HashMap`; its iteration order is randomized per
    // process. The fix sorts relationships by `(rel_type, target)` before
    // parsing secondary parts, so a given input must produce byte-identical
    // primitives across repeated decodes within the same process.
    //
    // We cannot trigger cross-process variation from within one test, but
    // we *can* decode the same bytes multiple times and require the
    // serialized primitive stream (positions + source_order + content) to
    // be bit-identical. That catches any residual HashMap leakage through
    // stateful iterators or `rand`-based code paths.
    let docx = build_docx_with_body_table_and_footer_table();
    let decoder = DocxDecoder::new();

    let baseline = decoder.decode(docx.clone()).unwrap();
    let baseline_json = serde_json::to_string(&baseline.primitives).unwrap();

    for iteration in 0..9 {
        let run = decoder.decode(docx.clone()).unwrap();
        let run_json = serde_json::to_string(&run.primitives).unwrap();
        assert_eq!(
            run_json, baseline_json,
            "primitive stream diverged on iteration {}",
            iteration
        );
    }
}

#[test]
fn multiple_secondary_parts_stack_without_overlap() {
    // When a DOCX references several secondary parts (e.g. a header *and*
    // a footer), each part must occupy a distinct y-range so successive
    // parts don't overwrite each other either. The fix advances
    // `secondary_part_y` past the last primitive it emitted; this test
    // locks in that property.
    let body = r#"<w:p><w:r><w:t>Body line</w:t></w:r></w:p>"#;

    let header_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:p><w:r><w:t>Header line</w:t></w:r></w:p>
</w:hdr>"#;

    let footer_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:p><w:r><w:t>Footer line</w:t></w:r></w:p>
</w:ftr>"#;

    let rels = [
        r#"<Relationship Id="rIdHeader1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/>"#,
        r#"<Relationship Id="rIdFooter1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/>"#,
    ];

    let docx = build_docx_with_parts_and_rels(
        body,
        None,
        None,
        &rels,
        &[
            ("word/header1.xml", header_xml),
            ("word/footer1.xml", footer_xml),
        ],
    );
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    let header = result
        .primitives
        .iter()
        .find(
            |p| matches!(&p.kind, PrimitiveKind::Text { content, .. } if content == "Header line"),
        )
        .expect("header primitive must be preserved");
    let footer = result
        .primitives
        .iter()
        .find(
            |p| matches!(&p.kind, PrimitiveKind::Text { content, .. } if content == "Footer line"),
        )
        .expect("footer primitive must be preserved");

    assert!(
        header
            .hints
            .iter()
            .any(|h| matches!(h.kind, HintKind::PageHeader)),
        "header primitive must carry PageHeader hint"
    );
    assert!(
        footer
            .hints
            .iter()
            .any(|h| matches!(h.kind, HintKind::PageFooter)),
        "footer primitive must carry PageFooter hint"
    );

    // Header precedes footer in the POI ordering (rel_type sort: Header=0,
    // Footer=1), so the footer's y must be past the header's y — otherwise
    // a second secondary part could overlap the first.
    assert!(
        footer.bbox.y > header.bbox.y + header.bbox.height,
        "footer y={} must sit past header y+h={} on page {} to avoid stacked-part overlap",
        footer.bbox.y,
        header.bbox.y + header.bbox.height,
        header.page
    );
}
