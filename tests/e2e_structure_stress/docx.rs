use olga::formats::docx::DocxDecoder;
use olga::model::NodeKind;
use olga::traits::FormatDecoder;

use crate::support::docx::{build_docx, build_docx_with_parts};
use crate::{collect_nodes, count_nodes, run_structure};

fn decode_docx(data: &[u8]) -> olga::traits::DecodeResult {
    DocxDecoder::new()
        .decode(data.to_vec())
        .expect("DOCX decode failed")
}

#[test]
fn stress_docx_heading_then_immediate_table() {
    let body = r#"
    <w:p>
      <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
      <w:r><w:t>Budget</w:t></w:r>
    </w:p>
    <w:tbl>
      <w:tr>
        <w:tc><w:p><w:r><w:t>Category</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>Amount</w:t></w:r></w:p></w:tc>
      </w:tr>
      <w:tr>
        <w:tc><w:p><w:r><w:t>Salaries</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>50000</w:t></w:r></w:p></w:tc>
      </w:tr>
      <w:tr>
        <w:tc><w:p><w:r><w:t>Equipment</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>15000</w:t></w:r></w:p></w:tc>
      </w:tr>
    </w:tbl>
    "#;
    let styles = Some(
        r#"<?xml version="1.0"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="Heading1">
    <w:name w:val="heading 1"/>
    <w:pPr><w:outlineLvl w:val="0"/></w:pPr>
  </w:style>
</w:styles>"#,
    );
    let docx = build_docx_with_parts(body, styles, None, &[]);
    let result = run_structure(decode_docx(&docx));

    let headings = collect_nodes(&result.root, &|k| matches!(k, NodeKind::Heading { .. }));
    assert!(
        !headings.is_empty(),
        "should have at least one heading, got 0"
    );
    assert!(
        headings[0].text().unwrap().contains("Budget"),
        "heading text should be 'Budget'"
    );

    let tables = collect_nodes(&result.root, &|k| matches!(k, NodeKind::Table { .. }));
    assert!(
        !tables.is_empty(),
        "table directly after heading must produce Table node"
    );

    let cells = collect_nodes(&result.root, &|k| matches!(k, NodeKind::TableCell { .. }));
    assert!(
        cells.len() >= 6,
        "expected 6 cells (3 rows x 2 cols), got {}",
        cells.len()
    );

    let cell_texts: Vec<&str> = cells.iter().filter_map(|n| n.text()).collect();
    assert!(
        cell_texts.iter().any(|t| t.contains("Salaries")),
        "cell 'Salaries' missing from table"
    );
    assert!(
        cell_texts.iter().any(|t| t.contains("Equipment")),
        "cell 'Equipment' missing from table"
    );
}

#[test]
fn stress_docx_two_consecutive_tables() {
    let body = r#"
    <w:tbl>
      <w:tr>
        <w:tc><w:p><w:r><w:t>A1</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>B1</w:t></w:r></w:p></w:tc>
      </w:tr>
    </w:tbl>
    <w:tbl>
      <w:tr>
        <w:tc><w:p><w:r><w:t>X1</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>Y1</w:t></w:r></w:p></w:tc>
      </w:tr>
    </w:tbl>
    "#;
    let docx = build_docx(body);
    let result = run_structure(decode_docx(&docx));

    let tables = collect_nodes(&result.root, &|k| matches!(k, NodeKind::Table { .. }));
    assert!(
        tables.len() >= 2,
        "two <w:tbl> blocks should produce at least 2 Table nodes, got {}",
        tables.len()
    );

    let full = result.root.all_text();
    assert!(full.contains("A1"), "first table content 'A1' missing");
    assert!(full.contains("X1"), "second table content 'X1' missing");
}

#[test]
fn stress_docx_table_with_empty_cells() {
    let body = r#"
    <w:tbl>
      <w:tr>
        <w:tc><w:p><w:r><w:t>Name</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>Value</w:t></w:r></w:p></w:tc>
      </w:tr>
      <w:tr>
        <w:tc><w:p><w:r><w:t>Alpha</w:t></w:r></w:p></w:tc>
        <w:tc><w:p/></w:tc>
      </w:tr>
      <w:tr>
        <w:tc><w:p/></w:tc>
        <w:tc><w:p><w:r><w:t>42</w:t></w:r></w:p></w:tc>
      </w:tr>
    </w:tbl>
    "#;
    let docx = build_docx(body);
    let result = run_structure(decode_docx(&docx));

    let tables = collect_nodes(&result.root, &|k| matches!(k, NodeKind::Table { .. }));
    assert!(!tables.is_empty(), "should produce a Table node");

    let full = result.root.all_text();
    assert!(full.contains("Name"), "'Name' missing");
    assert!(full.contains("Alpha"), "'Alpha' missing");
    assert!(full.contains("42"), "'42' missing");
}

#[test]
fn stress_docx_table_with_colspan() {
    let body = r#"
    <w:tbl>
      <w:tblGrid>
        <w:gridCol w:w="3000"/>
        <w:gridCol w:w="3000"/>
        <w:gridCol w:w="3000"/>
      </w:tblGrid>
      <w:tr>
        <w:tc>
          <w:tcPr><w:gridSpan w:val="3"/></w:tcPr>
          <w:p><w:r><w:t>Merged Header</w:t></w:r></w:p>
        </w:tc>
      </w:tr>
      <w:tr>
        <w:tc><w:p><w:r><w:t>Col1</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>Col2</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>Col3</w:t></w:r></w:p></w:tc>
      </w:tr>
    </w:tbl>
    "#;
    let docx = build_docx(body);
    let result = run_structure(decode_docx(&docx));

    let tables = collect_nodes(&result.root, &|k| matches!(k, NodeKind::Table { .. }));
    assert!(!tables.is_empty(), "should produce a Table node");

    let cells = collect_nodes(&result.root, &|k| matches!(k, NodeKind::TableCell { .. }));
    assert!(
        cells.len() >= 4,
        "expected at least 4 cells (1 merged + 3), got {}",
        cells.len()
    );

    let full = result.root.all_text();
    assert!(
        full.contains("Merged Header"),
        "'Merged Header' should survive"
    );
}

#[test]
fn stress_docx_deep_heading_hierarchy() {
    let styles = Some(
        r#"<?xml version="1.0"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="Heading1">
    <w:name w:val="heading 1"/>
    <w:pPr><w:outlineLvl w:val="0"/></w:pPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading2">
    <w:name w:val="heading 2"/>
    <w:pPr><w:outlineLvl w:val="1"/></w:pPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading3">
    <w:name w:val="heading 3"/>
    <w:pPr><w:outlineLvl w:val="2"/></w:pPr>
  </w:style>
</w:styles>"#,
    );
    let body = r#"
    <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Chapter One</w:t></w:r></w:p>
    <w:p><w:r><w:t>Intro paragraph for chapter one.</w:t></w:r></w:p>
    <w:p><w:pPr><w:pStyle w:val="Heading2"/></w:pPr><w:r><w:t>Section 1.1</w:t></w:r></w:p>
    <w:p><w:r><w:t>Content under section 1.1.</w:t></w:r></w:p>
    <w:p><w:pPr><w:pStyle w:val="Heading3"/></w:pPr><w:r><w:t>Subsection 1.1.1</w:t></w:r></w:p>
    <w:p><w:r><w:t>Deep content.</w:t></w:r></w:p>
    <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Chapter Two</w:t></w:r></w:p>
    <w:p><w:r><w:t>Intro paragraph for chapter two.</w:t></w:r></w:p>
    "#;
    let docx = build_docx_with_parts(body, styles, None, &[]);
    let result = run_structure(decode_docx(&docx));

    let headings = collect_nodes(&result.root, &|k| matches!(k, NodeKind::Heading { .. }));
    assert!(
        headings.len() >= 4,
        "should have 4 headings (H1, H2, H3, H1), got {}",
        headings.len()
    );

    let levels: Vec<u8> = headings
        .iter()
        .filter_map(|h| match &h.kind {
            NodeKind::Heading { level, .. } => Some(*level),
            _ => None,
        })
        .collect();
    assert!(
        levels.contains(&1) && levels.contains(&2) && levels.contains(&3),
        "should have levels 1, 2, and 3, got {:?}",
        levels
    );

    let full = result.root.all_text();
    assert!(full.contains("Chapter One"), "'Chapter One' missing");
    assert!(full.contains("Deep content"), "'Deep content' missing");
    assert!(full.contains("Chapter Two"), "'Chapter Two' missing");
}

#[test]
fn stress_docx_bold_paragraph_not_heading() {
    let body = r#"
    <w:p>
      <w:r>
        <w:rPr><w:b/></w:rPr>
        <w:t>Important note:</w:t>
      </w:r>
      <w:r>
        <w:t> This is a bold paragraph, not a heading. It has no outline level and should remain a paragraph.</w:t>
      </w:r>
    </w:p>
    <w:p><w:r><w:t>Normal paragraph following the bold one.</w:t></w:r></w:p>
    "#;
    let docx = build_docx(body);
    let dr = decode_docx(&docx);

    let heading_hints: Vec<_> = dr
        .primitives
        .iter()
        .filter(|p| {
            p.hints
                .iter()
                .any(|h| matches!(h.kind, olga::model::HintKind::Heading { .. }))
        })
        .collect();
    assert!(
        heading_hints.is_empty(),
        "bold paragraph without outlineLvl must NOT get Heading hint"
    );

    let result = crate::build_engine().structure(dr);

    let headings = count_nodes(&result.root, &|k| matches!(k, NodeKind::Heading { .. }));
    assert_eq!(
        headings, 0,
        "bold paragraph should not become a heading, got {} headings",
        headings
    );

    let paragraphs = count_nodes(&result.root, &|k| matches!(k, NodeKind::Paragraph { .. }));
    assert!(
        paragraphs >= 2,
        "should have at least 2 paragraphs, got {}",
        paragraphs
    );
}

#[test]
fn stress_docx_numbered_list() {
    let numbering = Some(
        r#"<?xml version="1.0"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="0">
    <w:lvl w:ilvl="0">
      <w:start w:val="1"/>
      <w:numFmt w:val="decimal"/>
    </w:lvl>
    <w:lvl w:ilvl="1">
      <w:start w:val="1"/>
      <w:numFmt w:val="lowerLetter"/>
    </w:lvl>
  </w:abstractNum>
  <w:num w:numId="1">
    <w:abstractNumId w:val="0"/>
  </w:num>
</w:numbering>"#,
    );
    let body = r#"
    <w:p><w:r><w:t>Here is a list:</w:t></w:r></w:p>
    <w:p>
      <w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr>
      <w:r><w:t>First item</w:t></w:r>
    </w:p>
    <w:p>
      <w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr>
      <w:r><w:t>Second item</w:t></w:r>
    </w:p>
    <w:p>
      <w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="1"/></w:numPr></w:pPr>
      <w:r><w:t>Nested item</w:t></w:r>
    </w:p>
    <w:p>
      <w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr>
      <w:r><w:t>Third item</w:t></w:r>
    </w:p>
    <w:p><w:r><w:t>Paragraph after list.</w:t></w:r></w:p>
    "#;
    let docx = build_docx_with_parts(body, None, numbering, &[]);
    let result = run_structure(decode_docx(&docx));

    let lists = collect_nodes(&result.root, &|k| matches!(k, NodeKind::List { .. }));
    assert!(
        !lists.is_empty(),
        "numbered list with numPr should produce List node"
    );

    let items = collect_nodes(&result.root, &|k| matches!(k, NodeKind::ListItem { .. }));
    assert!(
        items.len() >= 3,
        "expected at least 3 list items, got {}",
        items.len()
    );

    let item_texts: Vec<&str> = items.iter().filter_map(|n| n.text()).collect();
    assert!(
        item_texts.iter().any(|t| t.contains("First item")),
        "'First item' missing from list items"
    );
    assert!(
        item_texts.iter().any(|t| t.contains("Nested item")),
        "'Nested item' missing from list items"
    );

    let paras = collect_nodes(&result.root, &|k| matches!(k, NodeKind::Paragraph { .. }));
    let para_texts: Vec<&str> = paras.iter().filter_map(|n| n.text()).collect();
    assert!(
        para_texts
            .iter()
            .any(|t| t.contains("Paragraph after list")),
        "'Paragraph after list' should exist outside the list node"
    );
}

#[test]
fn stress_docx_kitchen_sink() {
    let styles = Some(
        r#"<?xml version="1.0"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="Heading1">
    <w:name w:val="heading 1"/>
    <w:pPr><w:outlineLvl w:val="0"/></w:pPr>
  </w:style>
</w:styles>"#,
    );
    let numbering = Some(
        r#"<?xml version="1.0"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="0">
    <w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="bullet"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>
</w:numbering>"#,
    );
    let body = r#"
    <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Overview</w:t></w:r></w:p>
    <w:p><w:r><w:t>This document tests all structural elements together.</w:t></w:r></w:p>
    <w:tbl>
      <w:tr>
        <w:tc><w:p><w:r><w:t>Metric</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>Value</w:t></w:r></w:p></w:tc>
      </w:tr>
      <w:tr>
        <w:tc><w:p><w:r><w:t>Users</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>1500</w:t></w:r></w:p></w:tc>
      </w:tr>
    </w:tbl>
    <w:p><w:r><w:t>Key takeaways:</w:t></w:r></w:p>
    <w:p>
      <w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr>
      <w:r><w:t>Growth is strong</w:t></w:r>
    </w:p>
    <w:p>
      <w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr>
      <w:r><w:t>Retention improved</w:t></w:r>
    </w:p>
    <w:p><w:r><w:t>Final remarks after the list.</w:t></w:r></w:p>
    "#;
    let docx = build_docx_with_parts(body, styles, numbering, &[]);
    let result = run_structure(decode_docx(&docx));

    let headings = count_nodes(&result.root, &|k| matches!(k, NodeKind::Heading { .. }));
    let paragraphs = count_nodes(&result.root, &|k| matches!(k, NodeKind::Paragraph { .. }));
    let tables = count_nodes(&result.root, &|k| matches!(k, NodeKind::Table { .. }));
    let lists = count_nodes(&result.root, &|k| matches!(k, NodeKind::List { .. }));

    assert!(headings >= 1, "should have headings, got {}", headings);
    assert!(
        paragraphs >= 2,
        "should have paragraphs, got {}",
        paragraphs
    );
    assert!(tables >= 1, "should have tables, got {}", tables);
    assert!(lists >= 1, "should have lists, got {}", lists);

    let total = result.root.node_count();
    assert!(
        total >= 10,
        "kitchen sink should have at least 10 nodes, got {}",
        total
    );

    let full = result.root.all_text();
    assert!(full.contains("Overview"), "'Overview' missing");
    assert!(full.contains("1500"), "'1500' missing from table");
    assert!(
        full.contains("Growth is strong"),
        "'Growth is strong' missing"
    );
    assert!(
        full.contains("Final remarks"),
        "'Final remarks' missing — list may have swallowed trailing paragraph"
    );
}

#[test]
fn stress_docx_empty_body() {
    let docx = build_docx("");
    let result = run_structure(decode_docx(&docx));

    assert!(matches!(result.root.kind, NodeKind::Document));
}

#[test]
fn stress_docx_very_long_paragraph() {
    let long_text = "word ".repeat(500);
    let body = format!("<w:p><w:r><w:t>{}</w:t></w:r></w:p>", long_text.trim());
    let docx = build_docx(&body);
    let result = run_structure(decode_docx(&docx));

    let paras = collect_nodes(&result.root, &|k| matches!(k, NodeKind::Paragraph { .. }));
    assert!(!paras.is_empty(), "long text should produce a paragraph");

    let text = paras[0].text().unwrap_or("");
    assert!(
        text.len() >= 2000,
        "paragraph text should be at least 2000 chars, got {}",
        text.len()
    );
}

#[test]
fn stress_docx_multi_run_paragraph_merges() {
    let body = r#"
    <w:p>
      <w:r><w:t>Hello </w:t></w:r>
      <w:r><w:rPr><w:b/></w:rPr><w:t>bold </w:t></w:r>
      <w:r><w:rPr><w:i/></w:rPr><w:t>italic </w:t></w:r>
      <w:r><w:t>normal.</w:t></w:r>
    </w:p>
    "#;
    let docx = build_docx(body);
    let result = run_structure(decode_docx(&docx));

    let paras = collect_nodes(&result.root, &|k| matches!(k, NodeKind::Paragraph { .. }));
    assert!(
        paras.len() <= 2,
        "multi-run paragraph should not explode into {} paragraphs",
        paras.len()
    );

    let full = result.root.all_text();
    assert!(full.contains("Hello"), "'Hello' missing");
    assert!(full.contains("bold"), "'bold' missing");
    assert!(full.contains("italic"), "'italic' missing");
    assert!(full.contains("normal"), "'normal' missing");
}
