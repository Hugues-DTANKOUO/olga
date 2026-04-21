use olga::formats::xlsx::XlsxDecoder;
use olga::model::{DocumentNode, NodeKind, StructureSource};
use olga::traits::FormatDecoder;

use crate::support::docx::{build_docx, build_docx_with_parts};
use crate::support::html::decode_html;
use crate::support::xlsx::build_xlsx;
use crate::{build_engine, run_structure};

#[test]
fn stress_all_synthetic_confidence_valid() {
    fn check_tree(node: &DocumentNode, label: &str) {
        assert!(
            (0.0..=1.0).contains(&node.confidence),
            "{}: confidence {} out of range for {:?}",
            label,
            node.confidence,
            node.kind
        );
        for child in &node.children {
            check_tree(child, label);
        }
    }

    let docx = build_docx(
        r#"<w:p><w:r><w:t>Hello</w:t></w:r></w:p>
        <w:tbl><w:tr><w:tc><w:p><w:r><w:t>Cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#,
    );
    let r = run_structure(
        olga::formats::docx::DocxDecoder::new()
            .decode(docx)
            .expect("DOCX decode failed"),
    );
    check_tree(&r.root, "DOCX-synthetic");

    let dr = decode_html(
        "<html><body><h1>Hi</h1><p>Text</p><table><tr><td>C</td></tr></table></body></html>",
    );
    let r = build_engine().structure(dr);
    check_tree(&r.root, "HTML-synthetic");

    let sheets: Vec<(&str, Vec<Vec<&str>>)> = vec![("S1", vec![vec!["A", "B"], vec!["1", "2"]])];
    let xlsx = build_xlsx(&sheets);
    let dr = XlsxDecoder::new().decode(xlsx).unwrap();
    let r = build_engine().structure(dr);
    check_tree(&r.root, "XLSX-synthetic");
}

#[test]
fn stress_all_synthetic_provenance_is_hint_assembled() {
    fn check_provenance(node: &DocumentNode, label: &str) {
        if !matches!(node.kind, NodeKind::Document) {
            assert!(
                matches!(
                    node.structure_source,
                    StructureSource::HintAssembled | StructureSource::HeuristicDetected { .. }
                ),
                "{}: unexpected provenance {:?} for {:?}",
                label,
                node.structure_source,
                node.kind
            );
        }
        for child in &node.children {
            check_provenance(child, label);
        }
    }

    let styles = Some(
        r#"<?xml version="1.0"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="Heading1">
    <w:name w:val="heading 1"/>
    <w:pPr><w:outlineLvl w:val="0"/></w:pPr>
  </w:style>
</w:styles>"#,
    );
    let body = r#"
    <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Title</w:t></w:r></w:p>
    <w:p><w:r><w:t>Body text.</w:t></w:r></w:p>
    "#;
    let docx = build_docx_with_parts(body, styles, None, &[]);
    let r = run_structure(
        olga::formats::docx::DocxDecoder::new()
            .decode(docx)
            .expect("DOCX decode failed"),
    );
    check_provenance(&r.root, "DOCX");

    let dr = decode_html("<html><body><h2>Section</h2><p>Content</p></body></html>");
    let r = build_engine().structure(dr);
    check_provenance(&r.root, "HTML");
}
