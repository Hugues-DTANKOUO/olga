use olga::formats::html::HtmlDecoder;
use olga::model::NodeKind;
use olga::traits::FormatDecoder;

use crate::{all_text_of, build_engine, count_nodes_by_kind};

#[test]
fn e2e_html_stress_decodes_and_structures() {
    let html = include_str!("../corpus/html/complex_report.html");
    let decoder = HtmlDecoder::new();
    let decode_result = decoder
        .decode(html.as_bytes().to_vec())
        .expect("HTML stress decode failed");

    assert!(
        decode_result.primitives.len() >= 30,
        "stress HTML should have many primitives, got {}",
        decode_result.primitives.len()
    );

    let result = build_engine().structure(decode_result);

    assert!(matches!(result.root.kind, NodeKind::Document));
    assert!(
        result.root.node_count() >= 30,
        "stress HTML should produce a complex tree, got {} nodes",
        result.root.node_count()
    );
}

#[test]
fn e2e_html_stress_has_all_structural_types() {
    let html = include_str!("../corpus/html/complex_report.html");
    let decoder = HtmlDecoder::new();
    let decode_result = decoder.decode(html.as_bytes().to_vec()).unwrap();
    let result = build_engine().structure(decode_result);

    let headings = count_nodes_by_kind(&result.root, &|k| matches!(k, NodeKind::Heading { .. }));
    let paragraphs =
        count_nodes_by_kind(&result.root, &|k| matches!(k, NodeKind::Paragraph { .. }));
    let tables = count_nodes_by_kind(&result.root, &|k| matches!(k, NodeKind::Table { .. }));
    let lists = count_nodes_by_kind(&result.root, &|k| matches!(k, NodeKind::List { .. }));
    let items = count_nodes_by_kind(&result.root, &|k| matches!(k, NodeKind::ListItem { .. }));

    assert!(
        headings >= 6,
        "stress HTML has h1 + 5xh2 + 2xh3 = 8 headings, got {}",
        headings
    );
    assert!(
        paragraphs >= 5,
        "should have paragraphs, got {}",
        paragraphs
    );
    assert!(tables >= 3, "should have 3 tables, got {}", tables);
    assert!(lists >= 2, "should have lists (ul + ol), got {}", lists);
    assert!(items >= 8, "should have many list items, got {}", items);
}

#[test]
fn e2e_html_stress_nested_list_depth() {
    let html = include_str!("../corpus/html/complex_report.html");
    let decoder = HtmlDecoder::new();
    let decode_result = decoder.decode(html.as_bytes().to_vec()).unwrap();
    let result = build_engine().structure(decode_result);

    let full = all_text_of(&result.root);
    assert!(
        full.contains("Project Aurora"),
        "'Project Aurora' (level 1) missing"
    );
    assert!(
        full.contains("Database migration"),
        "'Database migration' (level 2) missing"
    );
    assert!(
        full.contains("Design system audit"),
        "'Design system audit' (level 3) missing"
    );
    assert!(
        full.contains("Component library"),
        "'Component library' (level 3) missing"
    );
}

#[test]
fn e2e_html_stress_table_with_spans() {
    let html = include_str!("../corpus/html/complex_report.html");
    let decoder = HtmlDecoder::new();
    let decode_result = decoder.decode(html.as_bytes().to_vec()).unwrap();
    let result = build_engine().structure(decode_result);

    let full = all_text_of(&result.root);
    assert!(full.contains("FTE"), "'FTE' missing from spanned table");
    assert!(
        full.contains("Contractors"),
        "'Contractors' missing from spanned table"
    );
    assert!(full.contains("267"), "'267' (Engineering FTE) missing");
}

#[test]
fn e2e_html_stress_text_integrity() {
    let html = include_str!("../corpus/html/complex_report.html");
    let decoder = HtmlDecoder::new();
    let decode_result = decoder.decode(html.as_bytes().to_vec()).unwrap();
    let result = build_engine().structure(decode_result);

    let full = all_text_of(&result.root);

    assert!(full.contains("North America"), "'North America' missing");
    assert!(full.contains("Asia Pacific"), "'Asia Pacific' missing");
    assert!(full.contains("Project Aurora"), "'Project Aurora' missing");
    assert!(
        full.contains("Cost Optimization"),
        "'Cost Optimization' missing"
    );
    assert!(
        full.contains("vendor consolidation") || full.contains("Vendor consolidation"),
        "'vendor consolidation' missing"
    );
    assert!(
        full.contains("penetration testing"),
        "'penetration testing' missing from Q4 priorities"
    );
    assert!(
        full.contains("CONFIDENTIAL"),
        "bold disclaimer 'CONFIDENTIAL' missing"
    );
    assert!(
        full.contains("distributed tracing"),
        "'distributed tracing' missing from tech notes"
    );
}

#[test]
fn e2e_html_stress_no_heuristic_detection() {
    let html = include_str!("../corpus/html/complex_report.html");
    let decoder = HtmlDecoder::new();
    let decode_result = decoder.decode(html.as_bytes().to_vec()).unwrap();
    let result = build_engine().structure(decode_result);

    assert!(
        result.heuristic_pages.is_empty(),
        "HTML should not trigger heuristic detection, got: {:?}",
        result.heuristic_pages
    );
}
