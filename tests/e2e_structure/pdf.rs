use olga::formats::pdf::PdfDecoder;
use olga::model::NodeKind;
use olga::traits::FormatDecoder;

use crate::{all_text_of, build_engine, count_nodes_by_kind};

#[test]
fn e2e_pdf_structured_report_decodes_and_structures() {
    let data = include_bytes!("../corpus/pdf/structured_report.pdf").to_vec();
    let decoder = PdfDecoder;
    let decode_result = decoder.decode(data).expect("PDF decode should succeed");

    assert!(
        !decode_result.primitives.is_empty(),
        "should extract primitives"
    );
    assert!(
        decode_result.metadata.page_count >= 2,
        "should have at least 2 pages"
    );

    let engine = build_engine();
    let result = engine.structure(decode_result);

    assert!(matches!(result.root.kind, NodeKind::Document));
    assert!(
        !result.root.children.is_empty(),
        "PDF document should have children"
    );
}

#[test]
fn e2e_pdf_structured_report_contains_key_text() {
    let data = include_bytes!("../corpus/pdf/structured_report.pdf").to_vec();
    let decoder = PdfDecoder;
    let decode_result = decoder.decode(data).expect("PDF decode should succeed");
    let engine = build_engine();
    let result = engine.structure(decode_result);

    let full_text = all_text_of(&result.root);

    assert!(
        full_text.contains("Quarterly") || full_text.contains("quarterly"),
        "full text should contain 'Quarterly'"
    );
    assert!(
        full_text.contains("Aurora"),
        "full text should contain 'Aurora' (project name)"
    );
    assert!(
        full_text.contains("Risk") || full_text.contains("risk"),
        "full text should contain 'Risk'"
    );
}

#[test]
fn e2e_pdf_structured_report_spans_multiple_pages() {
    let data = include_bytes!("../corpus/pdf/structured_report.pdf").to_vec();
    let decoder = PdfDecoder;
    let decode_result = decoder.decode(data).expect("PDF decode should succeed");
    let engine = build_engine();
    let result = engine.structure(decode_result);

    assert!(
        result.root.source_pages.end >= 2,
        "document should span at least 2 pages, got {:?}",
        result.root.source_pages
    );
}

#[test]
fn e2e_pdf_structured_report_has_structural_variety() {
    let data = include_bytes!("../corpus/pdf/structured_report.pdf").to_vec();
    let decoder = PdfDecoder;
    let decode_result = decoder.decode(data).expect("PDF decode should succeed");
    let engine = build_engine();
    let result = engine.structure(decode_result);

    let paragraphs =
        count_nodes_by_kind(&result.root, &|k| matches!(k, NodeKind::Paragraph { .. }));
    assert!(
        paragraphs >= 2,
        "expected at least 2 paragraphs, got {}",
        paragraphs
    );

    let total_nodes = result.root.node_count();
    assert!(
        total_nodes >= 5,
        "expected at least 5 nodes in tree, got {}",
        total_nodes
    );
}

#[test]
fn e2e_pdf_structured_report_heuristic_detection_fired() {
    let data = include_bytes!("../corpus/pdf/structured_report.pdf").to_vec();
    let decoder = PdfDecoder;
    let decode_result = decoder.decode(data).expect("PDF decode should succeed");
    let engine = build_engine();
    let result = engine.structure(decode_result);

    assert!(
        !result.heuristic_pages.is_empty(),
        "untagged PDF should trigger heuristic detection"
    );
}
