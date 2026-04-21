use olga::model::NodeKind;

use crate::support::html::decode_html;
use crate::{build_engine, collect_nodes};

#[test]
fn stress_html_complex_nested_structure() {
    let html = r#"<!DOCTYPE html>
<html lang="en">
<head><title>Complex Page</title></head>
<body>
  <h1>Main Title</h1>
  <p>Introduction paragraph with some <strong>bold</strong> text.</p>

  <h2>Data Section</h2>
  <table>
    <thead>
      <tr><th>Product</th><th>Q1</th><th>Q2</th><th>Q3</th></tr>
    </thead>
    <tbody>
      <tr><td>Widget A</td><td>100</td><td>150</td><td>200</td></tr>
      <tr><td>Widget B</td><td>80</td><td>120</td><td>160</td></tr>
      <tr><td>Widget C</td><td>60</td><td>90</td><td>130</td></tr>
    </tbody>
  </table>

  <h2>Action Items</h2>
  <ul>
    <li>Review Q3 projections</li>
    <li>Update pricing model</li>
    <li>Schedule stakeholder meeting</li>
  </ul>

  <h3>Sub-actions</h3>
  <ol>
    <li>Draft agenda</li>
    <li>Send invitations</li>
  </ol>

  <p>Closing paragraph: all items tracked in the system.</p>
</body>
</html>"#;

    let dr = decode_html(html);
    let result = build_engine().structure(dr);

    assert!(matches!(result.root.kind, NodeKind::Document));

    let headings = collect_nodes(&result.root, &|k| matches!(k, NodeKind::Heading { .. }));
    assert!(
        headings.len() >= 3,
        "expected at least 3 headings, got {}",
        headings.len()
    );

    let tables = collect_nodes(&result.root, &|k| matches!(k, NodeKind::Table { .. }));
    assert!(!tables.is_empty(), "HTML table should produce Table node");

    let cells = collect_nodes(&result.root, &|k| matches!(k, NodeKind::TableCell { .. }));
    assert!(
        cells.len() >= 12,
        "expected at least 12 table cells, got {}",
        cells.len()
    );

    let lists = collect_nodes(&result.root, &|k| matches!(k, NodeKind::List { .. }));
    assert!(
        lists.len() >= 2,
        "expected at least 2 lists (ul + ol), got {}",
        lists.len()
    );

    let items = collect_nodes(&result.root, &|k| matches!(k, NodeKind::ListItem { .. }));
    assert!(
        items.len() >= 5,
        "expected at least 5 list items, got {}",
        items.len()
    );

    let full = result.root.all_text();
    assert!(full.contains("Widget A"), "'Widget A' missing");
    assert!(full.contains("200"), "'200' missing from table");
    assert!(
        full.contains("Review Q3 projections"),
        "'Review Q3 projections' missing from list"
    );
    assert!(
        full.contains("Closing paragraph"),
        "'Closing paragraph' missing — list may have swallowed it"
    );
}

#[test]
fn stress_html_table_with_spans() {
    let html = r#"<!DOCTYPE html>
<html><head><title>Spans</title></head>
<body>
  <table>
    <tr>
      <th colspan="2">Full Width Header</th>
    </tr>
    <tr>
      <td rowspan="2">Spanning Cell</td>
      <td>Right 1</td>
    </tr>
    <tr>
      <td>Right 2</td>
    </tr>
  </table>
</body>
</html>"#;

    let dr = decode_html(html);
    let result = build_engine().structure(dr);

    let tables = collect_nodes(&result.root, &|k| matches!(k, NodeKind::Table { .. }));
    assert!(!tables.is_empty(), "should produce a Table node");

    let full = result.root.all_text();
    assert!(
        full.contains("Full Width Header"),
        "'Full Width Header' missing"
    );
    assert!(full.contains("Spanning Cell"), "'Spanning Cell' missing");
    assert!(full.contains("Right 2"), "'Right 2' missing");
}

#[test]
fn stress_html_deeply_nested_list() {
    let html = r#"<!DOCTYPE html>
<html><head><title>Lists</title></head>
<body>
  <ul>
    <li>Level 1 Item A
      <ul>
        <li>Level 2 Item A1
          <ul>
            <li>Level 3 Item A1a</li>
            <li>Level 3 Item A1b</li>
          </ul>
        </li>
        <li>Level 2 Item A2</li>
      </ul>
    </li>
    <li>Level 1 Item B</li>
  </ul>
</body>
</html>"#;

    let dr = decode_html(html);
    let result = build_engine().structure(dr);

    let items = collect_nodes(&result.root, &|k| matches!(k, NodeKind::ListItem { .. }));
    assert!(
        items.len() >= 5,
        "expected at least 5 list items across 3 levels, got {}",
        items.len()
    );

    let full = result.root.all_text();
    assert!(
        full.contains("Level 3 Item A1a"),
        "deeply nested item missing"
    );
    assert!(
        full.contains("Level 1 Item B"),
        "top-level item after nesting missing"
    );
}

#[test]
fn stress_html_empty_body() {
    let html = r#"<!DOCTYPE html>
<html><head><title>Empty</title></head>
<body></body>
</html>"#;

    let dr = decode_html(html);
    let result = build_engine().structure(dr);

    assert!(matches!(result.root.kind, NodeKind::Document));
}

#[test]
fn stress_html_bare_text_only() {
    let html = r#"<!DOCTYPE html>
<html><head><title>Bare</title></head>
<body>
  Just some bare text with no semantic tags at all.
</body>
</html>"#;

    let dr = decode_html(html);
    let result = build_engine().structure(dr);

    let full = result.root.all_text();
    assert!(
        full.contains("bare text"),
        "bare text must survive: got '{}'",
        full
    );
}

#[test]
fn stress_html_consecutive_tables() {
    let html = r#"<!DOCTYPE html>
<html><head><title>Two Tables</title></head>
<body>
  <table>
    <tr><td>Table1-A</td><td>Table1-B</td></tr>
  </table>
  <table>
    <tr><td>Table2-X</td><td>Table2-Y</td></tr>
  </table>
</body>
</html>"#;

    let dr = decode_html(html);
    let result = build_engine().structure(dr);

    let tables = collect_nodes(&result.root, &|k| matches!(k, NodeKind::Table { .. }));
    assert!(
        tables.len() >= 2,
        "two <table> blocks should produce at least 2 Table nodes, got {}",
        tables.len()
    );

    let full = result.root.all_text();
    assert!(full.contains("Table1-A"), "first table content missing");
    assert!(full.contains("Table2-X"), "second table content missing");
}

#[test]
fn stress_html_inline_and_block_mixed() {
    let html = r#"<!DOCTYPE html>
<html><head><title>Mixed</title></head>
<body>
  <p>This has <strong>bold</strong>, <em>italic</em>, and <a href="https://example.com">a link</a>.</p>
  <blockquote>A profound quote from someone wise.</blockquote>
  <pre><code>fn main() { println!("hello"); }</code></pre>
  <p>Paragraph after code block.</p>
</body>
</html>"#;

    let dr = decode_html(html);
    let result = build_engine().structure(dr);

    let full = result.root.all_text();
    assert!(full.contains("bold"), "'bold' missing");
    assert!(full.contains("italic"), "'italic' missing");
    assert!(
        full.contains("Paragraph after code block"),
        "paragraph after code block missing"
    );

    let total = result.root.node_count();
    assert!(
        total >= 4,
        "mixed content should produce at least 4 nodes, got {}",
        total
    );
}
