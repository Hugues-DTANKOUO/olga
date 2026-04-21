use olga::formats::pdf::PdfDecoder;
use olga::model::NodeKind;
use olga::traits::FormatDecoder;

use crate::{all_text_of, build_engine, collect_nodes_by_kind, count_nodes_by_kind};

#[test]
fn e2e_pdf_stress_decodes_and_structures() {
    let data = include_bytes!("../corpus/pdf/multi_page_stress.pdf").to_vec();
    let decoder = PdfDecoder;
    let decode_result = decoder.decode(data).expect("PDF stress decode failed");

    assert!(
        decode_result.primitives.len() >= 50,
        "stress PDF should have many primitives, got {}",
        decode_result.primitives.len()
    );
    assert!(
        decode_result.metadata.page_count >= 5,
        "stress PDF should have 5 pages, got {}",
        decode_result.metadata.page_count
    );

    let result = build_engine().structure(decode_result);

    assert!(matches!(result.root.kind, NodeKind::Document));
    assert!(
        result.root.node_count() >= 15,
        "stress PDF should produce a substantial tree, got {} nodes",
        result.root.node_count()
    );
}

#[test]
fn e2e_pdf_stress_spans_all_pages() {
    let data = include_bytes!("../corpus/pdf/multi_page_stress.pdf").to_vec();
    let decoder = PdfDecoder;
    let decode_result = decoder.decode(data).unwrap();
    let result = build_engine().structure(decode_result);

    assert!(
        result.root.source_pages.end >= 5,
        "document should span all 5 pages, source_pages: {:?}",
        result.root.source_pages
    );
}

#[test]
fn e2e_pdf_stress_heuristic_fires() {
    let data = include_bytes!("../corpus/pdf/multi_page_stress.pdf").to_vec();
    let decoder = PdfDecoder;
    let decode_result = decoder.decode(data).unwrap();
    let result = build_engine().structure(decode_result);

    assert!(
        !result.heuristic_pages.is_empty(),
        "untagged PDF must trigger heuristic detection"
    );
}

#[test]
fn e2e_pdf_stress_text_integrity() {
    let data = include_bytes!("../corpus/pdf/multi_page_stress.pdf").to_vec();
    let decoder = PdfDecoder;
    let decode_result = decoder.decode(data).unwrap();
    let result = build_engine().structure(decode_result);

    let full = all_text_of(&result.root);

    assert!(
        full.contains("Infrastructure") || full.contains("infrastructure"),
        "'Infrastructure' missing from page 1 title"
    );
    assert!(
        full.contains("4.7 million") || full.contains("$4.7"),
        "budget figure missing from page 1"
    );
    assert!(
        full.contains("ERP") || full.contains("CRM"),
        "workload names missing from page 2 table"
    );
    assert!(
        full.contains("99.99%") || full.contains("uptime"),
        "network requirement text missing from page 3"
    );
    assert!(
        full.contains("CIS benchmarks") || full.contains("NIST"),
        "security hardening text missing from page 4"
    );
    assert!(
        full.contains("Recommendations") || full.contains("recommendations"),
        "'Recommendations' missing from page 5"
    );
}

#[test]
fn e2e_pdf_stress_has_paragraphs() {
    let data = include_bytes!("../corpus/pdf/multi_page_stress.pdf").to_vec();
    let decoder = PdfDecoder;
    let decode_result = decoder.decode(data).unwrap();
    let result = build_engine().structure(decode_result);

    let paragraphs =
        count_nodes_by_kind(&result.root, &|k| matches!(k, NodeKind::Paragraph { .. }));
    assert!(
        paragraphs >= 5,
        "stress PDF should produce many paragraphs, got {}",
        paragraphs
    );
}

#[test]
fn e2e_pdf_stress_heading_detection() {
    let data = include_bytes!("../corpus/pdf/multi_page_stress.pdf").to_vec();
    let decoder = PdfDecoder;
    let decode_result = decoder.decode(data).unwrap();
    let result = build_engine().structure(decode_result);

    let headings = collect_nodes_by_kind(&result.root, &|k| matches!(k, NodeKind::Heading { .. }));
    assert!(
        headings.len() >= 2,
        "stress PDF should detect headings from font size, got {}",
        headings.len()
    );
}

#[test]
fn e2e_pdf_stress_confidence_valid() {
    let data = include_bytes!("../corpus/pdf/multi_page_stress.pdf").to_vec();
    let decoder = PdfDecoder;
    let decode_result = decoder.decode(data).unwrap();
    let result = build_engine().structure(decode_result);

    fn check(node: &olga::model::DocumentNode) {
        assert!(
            (0.0..=1.0).contains(&node.confidence),
            "confidence {} out of range for {:?}",
            node.confidence,
            node.kind
        );
        for child in &node.children {
            check(child);
        }
    }
    check(&result.root);
}
