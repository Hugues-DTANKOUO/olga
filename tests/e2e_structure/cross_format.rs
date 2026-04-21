use olga::formats::docx::DocxDecoder;
use olga::formats::html::HtmlDecoder;
use olga::formats::pdf::PdfDecoder;
use olga::formats::xlsx::XlsxDecoder;
use olga::model::NodeKind;
use olga::traits::FormatDecoder;

use crate::{build_engine, collect_nodes_by_kind};

#[test]
fn e2e_all_formats_produce_document_root() {
    let cases: Vec<(&str, Box<dyn FormatDecoder>, Vec<u8>)> = vec![
        (
            "DOCX",
            Box::new(DocxDecoder::new()),
            include_bytes!("../corpus/docx/project_status.docx").to_vec(),
        ),
        (
            "PDF",
            Box::new(PdfDecoder),
            include_bytes!("../corpus/pdf/structured_report.pdf").to_vec(),
        ),
        (
            "XLSX",
            Box::new(XlsxDecoder::new()),
            include_bytes!("../corpus/xlsx/employee_directory.xlsx").to_vec(),
        ),
        (
            "HTML",
            Box::new(HtmlDecoder::new()),
            include_str!("../corpus/html/semantic_email.html")
                .as_bytes()
                .to_vec(),
        ),
    ];

    let engine = build_engine();

    for (name, decoder, data) in cases {
        let decode_result = decoder
            .decode(data)
            .unwrap_or_else(|e| panic!("{} decode failed: {:?}", name, e));

        let prim_count = decode_result.primitives.len();
        assert!(prim_count > 0, "{}: should produce primitives, got 0", name);

        let result = engine.structure(decode_result);

        assert!(
            matches!(result.root.kind, NodeKind::Document),
            "{}: root should be Document",
            name
        );
        assert!(
            !result.root.children.is_empty(),
            "{}: document should have children (from {} primitives)",
            name,
            prim_count
        );
        assert!(
            result.root.node_count() >= 3,
            "{}: tree should have at least 3 nodes, got {}",
            name,
            result.root.node_count()
        );

        let text_nodes = collect_nodes_by_kind(&result.root, &|k| {
            matches!(
                k,
                NodeKind::Paragraph { .. }
                    | NodeKind::Heading { .. }
                    | NodeKind::ListItem { .. }
                    | NodeKind::TableCell { .. }
            )
        });
        for node in &text_nodes {
            if let Some(text) = node.text() {
                assert!(
                    !text.is_empty(),
                    "{}: text node should not have empty text: {:?}",
                    name,
                    node.kind
                );
            }
        }
    }
}

#[test]
fn e2e_all_formats_confidence_in_valid_range() {
    let cases: Vec<(&str, Box<dyn FormatDecoder>, Vec<u8>)> = vec![
        (
            "DOCX",
            Box::new(DocxDecoder::new()),
            include_bytes!("../corpus/docx/project_status.docx").to_vec(),
        ),
        (
            "PDF",
            Box::new(PdfDecoder),
            include_bytes!("../corpus/pdf/structured_report.pdf").to_vec(),
        ),
        (
            "XLSX",
            Box::new(XlsxDecoder::new()),
            include_bytes!("../corpus/xlsx/employee_directory.xlsx").to_vec(),
        ),
    ];

    let engine = build_engine();

    for (name, decoder, data) in cases {
        let decode_result = decoder.decode(data).unwrap();
        let result = engine.structure(decode_result);

        fn check_confidence(node: &olga::model::DocumentNode, name: &str) {
            assert!(
                (0.0..=1.0).contains(&node.confidence),
                "{}: confidence {} out of range for {:?}",
                name,
                node.confidence,
                node.kind
            );
            for child in &node.children {
                check_confidence(child, name);
            }
        }

        check_confidence(&result.root, name);
    }
}
