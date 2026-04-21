use super::*;

fn decode_html(html: &str) -> DecodeResult {
    let decoder = HtmlDecoder::new();
    decoder
        .decode(html.as_bytes().to_vec())
        .expect("decode failed")
}

#[test]
fn decode_empty_html() {
    let result = decode_html("<html><body></body></html>");
    assert!(result.primitives.is_empty());
}

#[test]
fn decode_simple_paragraph() {
    let result = decode_html("<html><body><p>Hello world</p></body></html>");
    assert_eq!(result.primitives.len(), 1);
    assert_eq!(result.primitives[0].text_content(), Some("Hello world"));
    assert!(
        result.primitives[0]
            .hints
            .iter()
            .any(|h| matches!(h.kind, HintKind::Paragraph))
    );
}

#[test]
fn decode_heading_levels() {
    let html = r#"<html><body>
        <h1>Title</h1>
        <h2>Subtitle</h2>
        <h3>Section</h3>
    </body></html>"#;
    let result = decode_html(html);
    assert_eq!(result.primitives.len(), 3);
    assert!(
        result.primitives[0]
            .hints
            .iter()
            .any(|h| matches!(h.kind, HintKind::Heading { level: 1 }))
    );
    assert!(
        result.primitives[1]
            .hints
            .iter()
            .any(|h| matches!(h.kind, HintKind::Heading { level: 2 }))
    );
    assert!(
        result.primitives[2]
            .hints
            .iter()
            .any(|h| matches!(h.kind, HintKind::Heading { level: 3 }))
    );
}

#[test]
fn decode_title_extraction() {
    let html = "<html><head><title>My Page</title></head><body></body></html>";
    let result = decode_html(html);
    assert_eq!(result.metadata.title, Some("My Page".to_string()));
}

#[test]
fn decode_whitespace_collapsing() {
    let html = "<html><body><p>  Hello   \n\n  world  </p></body></html>";
    let result = decode_html(html);
    assert_eq!(result.primitives[0].text_content(), Some("Hello world"));
}

#[test]
fn decode_pre_preserves_whitespace() {
    let html = "<html><body><pre>  line1\n  line2</pre></body></html>";
    let result = decode_html(html);
    let text = result.primitives[0].text_content().unwrap();
    assert!(text.contains('\n'));
    assert!(text.contains("  line1"));
}

#[test]
fn decode_skips_script_and_style() {
    let html = r#"<html><body>
        <script>var x = 1;</script>
        <style>body { color: red; }</style>
        <p>Visible</p>
    </body></html>"#;
    let result = decode_html(html);
    assert_eq!(result.primitives.len(), 1);
    assert_eq!(result.primitives[0].text_content(), Some("Visible"));
}

#[test]
fn decode_unordered_list() {
    let html = r#"<html><body>
        <ul>
            <li>Item 1</li>
            <li>Item 2</li>
        </ul>
    </body></html>"#;
    let result = decode_html(html);
    assert_eq!(result.primitives.len(), 2);
    assert!(result.primitives[0].hints.iter().any(|h| matches!(
        h.kind,
        HintKind::ListItem {
            depth: 0,
            ordered: false,
            ..
        }
    )));
}

#[test]
fn decode_ordered_list() {
    let html = r#"<html><body>
        <ol>
            <li>First</li>
            <li>Second</li>
        </ol>
    </body></html>"#;
    let result = decode_html(html);
    assert!(result.primitives[0].hints.iter().any(|h| matches!(
        h.kind,
        HintKind::ListItem {
            depth: 0,
            ordered: true,
            ..
        }
    )));
}

#[test]
fn decode_nested_list() {
    let html = r#"<html><body>
        <ul>
            <li>Outer
                <ul>
                    <li>Inner</li>
                </ul>
            </li>
        </ul>
    </body></html>"#;
    let result = decode_html(html);
    assert!(result.primitives.len() >= 2);
    let inner = result
        .primitives
        .iter()
        .find(|p| p.text_content().is_some_and(|t| t.contains("Inner")));
    assert!(inner.is_some());
    assert!(
        inner
            .unwrap()
            .hints
            .iter()
            .any(|h| matches!(h.kind, HintKind::ListItem { depth: 1, .. }))
    );
}

#[test]
fn decode_simple_table() {
    let html = r#"<html><body>
        <table>
            <tr><th>Name</th><th>Age</th></tr>
            <tr><td>Alice</td><td>30</td></tr>
        </table>
    </body></html>"#;
    let result = decode_html(html);
    assert_eq!(result.primitives.len(), 4);
    assert!(
        result.primitives[0]
            .hints
            .iter()
            .any(|h| matches!(h.kind, HintKind::TableHeader { col: 0 }))
    );
    assert!(
        result.primitives[1]
            .hints
            .iter()
            .any(|h| matches!(h.kind, HintKind::TableHeader { col: 1 }))
    );
    assert!(
        result.primitives[2]
            .hints
            .iter()
            .any(|h| matches!(h.kind, HintKind::TableCell { row: 1, col: 0, .. }))
    );
}

#[test]
fn decode_table_with_colspan() {
    let html = r#"<html><body>
        <table>
            <tr><td colspan="2">Merged</td><td>C</td></tr>
            <tr><td>A</td><td>B</td><td>C2</td></tr>
        </table>
    </body></html>"#;
    let result = decode_html(html);
    let merged = &result.primitives[0];
    assert!(merged.hints.iter().any(|h| matches!(
        h.kind,
        HintKind::TableCell {
            row: 0,
            col: 0,
            colspan: 2,
            rowspan: 1
        }
    )));
    let c = &result.primitives[1];
    assert!(
        c.hints
            .iter()
            .any(|h| matches!(h.kind, HintKind::TableCell { col: 2, .. }))
    );
}

#[test]
fn decode_table_with_rowspan() {
    let html = r#"<html><body>
        <table>
            <tr><td rowspan="2">Span</td><td>B1</td></tr>
            <tr><td>B2</td></tr>
        </table>
    </body></html>"#;
    let result = decode_html(html);
    let b2 = result
        .primitives
        .iter()
        .find(|p| p.text_content() == Some("B2"))
        .unwrap();
    assert!(
        b2.hints
            .iter()
            .any(|h| matches!(h.kind, HintKind::TableCell { row: 1, col: 1, .. }))
    );
}

#[test]
fn decode_blockquote() {
    let html = "<html><body><blockquote>Quoted text</blockquote></body></html>";
    let result = decode_html(html);
    assert_eq!(result.primitives.len(), 1);
    assert!(
        result.primitives[0]
            .hints
            .iter()
            .any(|h| matches!(h.kind, HintKind::BlockQuote))
    );
}

#[test]
fn decode_image_with_alt() {
    let html = r#"<html><body><img src="pic.jpg" alt="A photo"></body></html>"#;
    let result = decode_html(html);
    assert_eq!(result.primitives.len(), 1);
    match &result.primitives[0].kind {
        PrimitiveKind::Image { alt_text, .. } => {
            assert_eq!(alt_text.as_deref(), Some("A photo"));
        }
        _ => panic!("Expected Image primitive"),
    }
}

#[test]
fn decode_link() {
    let html = r#"<html><body><a href="https://example.com">Click here</a></body></html>"#;
    let result = decode_html(html);
    assert_eq!(result.primitives.len(), 1);
    assert!(result.primitives[0].hints.iter().any(|h| matches!(
        &h.kind,
        HintKind::Link { url } if url == "https://example.com"
    )));
}

#[test]
fn decode_lang_from_html_tag() {
    let html = r#"<html lang="fr-FR"><body><p>Bonjour</p></body></html>"#;
    let result = decode_html(html);
    match &result.primitives[0].kind {
        PrimitiveKind::Text { lang, .. } => {
            assert_eq!(lang.as_deref(), Some("fr-FR"));
        }
        _ => panic!("Expected Text primitive"),
    }
}

#[test]
fn decode_malformed_html_no_crash() {
    let html = "<html><body><p>Unclosed paragraph<div>Misplaced div</p></div></body>";
    let result = decode_html(html);
    assert!(!result.primitives.is_empty());
}

#[test]
fn all_primitives_are_page_zero() {
    let html = "<html><body><h1>Title</h1><p>Text</p></body></html>";
    let result = decode_html(html);
    for prim in &result.primitives {
        assert_eq!(prim.page, 0);
    }
}

#[test]
fn source_order_is_monotonic() {
    let html = r#"<html><body>
        <h1>Title</h1>
        <p>P1</p>
        <p>P2</p>
        <ul><li>Item</li></ul>
    </body></html>"#;
    let result = decode_html(html);
    for window in result.primitives.windows(2) {
        assert!(window[0].source_order < window[1].source_order);
    }
}

#[test]
fn decode_section_markers() {
    let html = r#"<html><body><section><p>Inside</p></section></body></html>"#;
    let result = decode_html(html);
    assert_eq!(result.primitives.len(), 3);
    assert!(
        result.primitives[0]
            .hints
            .iter()
            .any(|h| matches!(h.kind, HintKind::SectionStart { .. }))
    );
    assert!(
        result.primitives[2]
            .hints
            .iter()
            .any(|h| matches!(h.kind, HintKind::SectionEnd))
    );
}

#[test]
fn decode_br_as_newline() {
    let html = "<html><body><p>Line 1<br>Line 2</p></body></html>";
    let result = decode_html(html);
    let text = result.primitives[0].text_content().unwrap();
    assert!(text.contains("Line 1"));
    assert!(text.contains("Line 2"));
}

#[test]
fn decode_definition_list() {
    let html = r#"<html><body>
        <dl>
            <dt>Term</dt>
            <dd>Definition</dd>
        </dl>
    </body></html>"#;
    let result = decode_html(html);
    assert_eq!(result.primitives.len(), 2);
    match &result.primitives[0].kind {
        PrimitiveKind::Text { is_bold, .. } => assert!(*is_bold),
        _ => panic!("Expected Text"),
    }
}
