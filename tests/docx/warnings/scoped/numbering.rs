use super::*;

#[test]
fn decode_unresolved_numbering_warns() {
    let body = r#"
    <w:p>
      <w:pPr>
        <w:numPr>
          <w:ilvl w:val="0"/>
          <w:numId w:val="42"/>
        </w:numPr>
      </w:pPr>
      <w:r><w:t>List item without numbering.xml</w:t></w:r>
    </w:p>
    "#;

    let docx = build_docx(body);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.kind == WarningKind::PartialExtraction)
    );
    assert!(
        result.primitives[0]
            .hints
            .iter()
            .any(|h| matches!(h.kind, HintKind::Paragraph))
    );
}

#[test]
fn decode_repeated_unresolved_numbering_in_header_cell_emit_summary_warning() {
    let header_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:tbl>
    <w:tr>
      <w:tc>
        <w:p>
          <w:pPr>
            <w:numPr>
              <w:ilvl w:val="0"/>
              <w:numId w:val="42"/>
            </w:numPr>
          </w:pPr>
          <w:r><w:t>Missing list 1</w:t></w:r>
        </w:p>
        <w:p>
          <w:pPr>
            <w:numPr>
              <w:ilvl w:val="0"/>
              <w:numId w:val="42"/>
            </w:numPr>
          </w:pPr>
          <w:r><w:t>Missing list 2</w:t></w:r>
        </w:p>
      </w:tc>
    </w:tr>
  </w:tbl>
</w:hdr>"#;

    let docx = build_docx_with_parts_and_rels(
        r#"<w:p><w:r><w:t>Body text</w:t></w:r></w:p>"#,
        None,
        None,
        &[
            r#"<Relationship Id="rIdHeader1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/>"#,
        ],
        &[("word/header1.xml", header_xml)],
    );
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert!(result.warnings.iter().any(|warning| {
        warning.kind == WarningKind::PartialExtraction
            && warning.message.contains(
                "header story table cell (0, 0) contains 2 paragraphs referencing unresolved numbering definitions",
            )
    }));
}
