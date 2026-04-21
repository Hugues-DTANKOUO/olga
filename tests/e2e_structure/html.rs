use olga::formats::html::HtmlDecoder;
use olga::model::NodeKind;
use olga::traits::FormatDecoder;

use crate::{all_text_of, build_engine, count_nodes_by_kind};

#[test]
fn e2e_html_semantic_email_decodes_and_structures() {
    let html = include_str!("../corpus/html/semantic_email.html");
    let decoder = HtmlDecoder::new();
    let decode_result = decoder
        .decode(html.as_bytes().to_vec())
        .expect("HTML decode should succeed");

    assert!(
        !decode_result.primitives.is_empty(),
        "should extract primitives from HTML"
    );

    let engine = build_engine();
    let result = engine.structure(decode_result);

    assert!(matches!(result.root.kind, NodeKind::Document));
    assert!(
        !result.root.children.is_empty(),
        "HTML document should have children"
    );
    assert!(
        result.heuristic_pages.is_empty(),
        "HTML should not trigger heuristic detection, but got pages: {:?}",
        result.heuristic_pages
    );
}

#[test]
fn e2e_html_semantic_email_has_headings_and_paragraphs() {
    let html = include_str!("../corpus/html/semantic_email.html");
    let decoder = HtmlDecoder::new();
    let decode_result = decoder
        .decode(html.as_bytes().to_vec())
        .expect("HTML decode should succeed");
    let engine = build_engine();
    let result = engine.structure(decode_result);

    let headings = count_nodes_by_kind(&result.root, &|k| matches!(k, NodeKind::Heading { .. }));
    let paragraphs =
        count_nodes_by_kind(&result.root, &|k| matches!(k, NodeKind::Paragraph { .. }));

    assert!(
        headings >= 1,
        "HTML email should have at least 1 heading, got {}",
        headings
    );
    assert!(
        paragraphs >= 1,
        "HTML email should have at least 1 paragraph, got {}",
        paragraphs
    );

    let full_text = all_text_of(&result.root);
    assert!(
        full_text.contains("Customer Update"),
        "HTML should contain 'Customer Update'"
    );
}
