use olga::formats::docx::DocxDecoder;
use olga::model::*;
use olga::traits::FormatDecoder;

use crate::support::docx::{build_docx, build_docx_with_parts};

#[test]
fn decode_metadata() {
    let docx = build_docx("<w:p><w:r><w:t>Test</w:t></w:r></w:p>");
    let decoder = DocxDecoder::new();
    let meta = decoder.metadata(&docx).unwrap();

    assert_eq!(meta.format, DocumentFormat::Docx);
    assert!(!meta.encrypted);
    assert!(meta.text_extraction_allowed);
    assert!(meta.is_processable());
}

#[test]
fn decode_metadata_uses_app_xml_page_estimate() {
    let app_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
  <Pages>7</Pages>
</Properties>"#;
    let docx = build_docx_with_parts(
        "<w:p><w:r><w:t>Test</w:t></w:r></w:p>",
        None,
        None,
        &[("docProps/app.xml", app_xml)],
    );
    let decoder = DocxDecoder::new();
    let meta = decoder.metadata(&docx).unwrap();

    assert_eq!(meta.page_count, 7);
    assert_eq!(
        meta.page_count_provenance.basis,
        PaginationBasis::ApplicationMetadata
    );
}

#[test]
fn decode_metadata_uses_rendered_page_break_markers() {
    let body = r#"
    <w:p><w:r><w:t>Page 1</w:t></w:r></w:p>
    <w:p><w:r><w:lastRenderedPageBreak/><w:t>Page 2</w:t></w:r></w:p>
    <w:p><w:r><w:lastRenderedPageBreak/><w:t>Page 3</w:t></w:r></w:p>
    "#;
    let docx = build_docx(body);
    let decoder = DocxDecoder::new();
    let meta = decoder.metadata(&docx).unwrap();

    assert_eq!(meta.page_count, 3);
    assert_eq!(
        meta.page_count_provenance.basis,
        PaginationBasis::RenderedPageBreakMarkers
    );
}

#[test]
fn decode_not_a_docx() {
    let decoder = DocxDecoder::new();
    let result = decoder.metadata(b"This is not a ZIP file");
    assert!(result.is_err());
}
