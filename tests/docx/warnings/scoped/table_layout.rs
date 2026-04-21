use super::*;

#[test]
fn decode_vmerge_warning_is_story_and_cell_scoped() {
    let header_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:tbl>
    <w:tr>
      <w:tc>
        <w:tcPr><w:vMerge w:val="continue"/></w:tcPr>
        <w:p><w:r><w:t>Continuation</w:t></w:r></w:p>
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
            && warning
                .message
                .contains("header story table cell (0, 0) continues a vertical merge without a matching restart cell")
    }));
}

#[test]
fn decode_repeated_vmerge_without_restart_in_header_story_emit_summary_warning() {
    let header_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:tbl>
    <w:tr>
      <w:tc>
        <w:tcPr><w:vMerge w:val="continue"/></w:tcPr>
        <w:p><w:r><w:t>Cell 1</w:t></w:r></w:p>
      </w:tc>
    </w:tr>
    <w:tr>
      <w:tc>
        <w:tcPr><w:vMerge w:val="continue"/></w:tcPr>
        <w:p><w:r><w:t>Cell 2</w:t></w:r></w:p>
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
            && warning
                .message
                .contains("header story contains 2 vertical merge continuations without a matching restart cell")
    }));
}

#[test]
fn decode_repeated_vmerge_colspan_mismatch_in_header_story_emit_summary_warning() {
    let header_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:tbl>
    <w:tr>
      <w:tc>
        <w:tcPr><w:vMerge w:val="restart"/></w:tcPr>
        <w:p><w:r><w:t>Restart 1</w:t></w:r></w:p>
      </w:tc>
    </w:tr>
    <w:tr>
      <w:tc>
        <w:tcPr>
          <w:gridSpan w:val="2"/>
          <w:vMerge w:val="continue"/>
        </w:tcPr>
        <w:p><w:r><w:t>Continue 1</w:t></w:r></w:p>
      </w:tc>
    </w:tr>
    <w:tr>
      <w:tc>
        <w:tcPr><w:vMerge w:val="restart"/></w:tcPr>
        <w:p><w:r><w:t>Restart 2</w:t></w:r></w:p>
      </w:tc>
    </w:tr>
    <w:tr>
      <w:tc>
        <w:tcPr>
          <w:gridSpan w:val="2"/>
          <w:vMerge w:val="continue"/>
        </w:tcPr>
        <w:p><w:r><w:t>Continue 2</w:t></w:r></w:p>
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
                "header story contains 2 vertical merge continuations whose colspan differs from the restart cell",
            )
    }));
}
