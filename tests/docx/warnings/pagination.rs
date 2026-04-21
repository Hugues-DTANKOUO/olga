use olga::error::WarningKind;
use olga::formats::docx::DocxDecoder;
use olga::model::*;
use olga::traits::FormatDecoder;

use crate::support::docx::build_docx_with_parts;

#[test]
fn decode_page_count_approximation_warns() {
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
    let result = decoder.decode(docx).unwrap();

    assert_eq!(result.metadata.page_count, 7);
    assert_eq!(
        result.metadata.page_count_provenance.basis,
        PaginationBasis::ApplicationMetadata
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.kind == WarningKind::ApproximatePagination)
    );
}

#[test]
fn decode_rendered_page_break_markers_override_app_xml() {
    // Under the max-wins pagination policy (see resolve_docx_page_count),
    // rendered markers "override" app.xml specifically when they report a
    // higher count — i.e. when Word's last layout captured more pages than
    // the cached `<Pages>` hint. Seven `w:lastRenderedPageBreak` markers
    // imply eight rendered pages, beating app.xml's 7.
    let app_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
  <Pages>7</Pages>
</Properties>"#;
    let body = r#"
    <w:p><w:r><w:t>Page 1</w:t></w:r></w:p>
    <w:p><w:r><w:lastRenderedPageBreak/><w:t>Page 2</w:t></w:r></w:p>
    <w:p><w:r><w:lastRenderedPageBreak/><w:t>Page 3</w:t></w:r></w:p>
    <w:p><w:r><w:lastRenderedPageBreak/><w:t>Page 4</w:t></w:r></w:p>
    <w:p><w:r><w:lastRenderedPageBreak/><w:t>Page 5</w:t></w:r></w:p>
    <w:p><w:r><w:lastRenderedPageBreak/><w:t>Page 6</w:t></w:r></w:p>
    <w:p><w:r><w:lastRenderedPageBreak/><w:t>Page 7</w:t></w:r></w:p>
    <w:p><w:r><w:lastRenderedPageBreak/><w:t>Page 8</w:t></w:r></w:p>
    "#;
    let docx = build_docx_with_parts(body, None, None, &[("docProps/app.xml", app_xml)]);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert_eq!(result.metadata.page_count, 8);
    assert_eq!(
        result.metadata.page_count_provenance.basis,
        PaginationBasis::RenderedPageBreakMarkers
    );
    assert!(result.warnings.iter().any(|warning| {
        warning.kind == WarningKind::ApproximatePagination
            && warning.message.contains("w:lastRenderedPageBreak")
            && warning.message.contains("docProps/app.xml")
    }));
}
