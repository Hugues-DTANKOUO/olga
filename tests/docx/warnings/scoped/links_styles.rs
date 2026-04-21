use super::*;

#[test]
fn decode_unresolved_hyperlink_warns() {
    let body = r#"
    <w:p>
      <w:hyperlink r:id="rId404">
        <w:r><w:t>Broken link</w:t></w:r>
      </w:hyperlink>
    </w:p>
    "#;

    let docx = build_docx(body);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.kind == WarningKind::UnresolvedRelationship)
    );
}

#[test]
fn decode_unresolved_hyperlink_warning_is_story_and_cell_scoped() {
    let header_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:tbl>
    <w:tr>
      <w:tc>
        <w:p>
          <w:hyperlink r:id="rId404">
            <w:r><w:t>Broken link</w:t></w:r>
          </w:hyperlink>
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
        warning.kind == WarningKind::UnresolvedRelationship
            && warning.message.contains("header story table cell (0, 0)")
            && warning.message.contains("rId404")
    }));
}

#[test]
fn decode_repeated_internal_hyperlink_relationships_in_header_cell_emit_summary_warning() {
    let header_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:tbl>
    <w:tr>
      <w:tc>
        <w:p>
          <w:hyperlink r:id="rIdInternal1">
            <w:r><w:t>Internal 1</w:t></w:r>
          </w:hyperlink>
        </w:p>
        <w:p>
          <w:hyperlink r:id="rIdInternal2">
            <w:r><w:t>Internal 2</w:t></w:r>
          </w:hyperlink>
        </w:p>
      </w:tc>
    </w:tr>
  </w:tbl>
</w:hdr>"#;
    let header_rels = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdInternal1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="bookmark-one"/>
  <Relationship Id="rIdInternal2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="bookmark-two"/>
</Relationships>"#;

    let docx = build_docx_with_parts_and_rels(
        r#"<w:p><w:r><w:t>Body text</w:t></w:r></w:p>"#,
        None,
        None,
        &[
            r#"<Relationship Id="rIdHeader1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/>"#,
        ],
        &[
            ("word/header1.xml", header_xml),
            ("word/_rels/header1.xml.rels", header_rels),
        ],
    );
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert!(result.warnings.iter().any(|warning| {
        warning.kind == WarningKind::PartialExtraction
            && warning.message.contains(
                "header story table cell (0, 0) contains 2 internal hyperlink relationships that are not surfaced as link hints",
            )
    }));
}

#[test]
fn decode_unresolved_style_warning_is_story_and_cell_scoped() {
    let header_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:tbl>
    <w:tr>
      <w:tc>
        <w:p>
          <w:pPr><w:pStyle w:val="MissingStyle"/></w:pPr>
          <w:r><w:t>Styled text</w:t></w:r>
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
        warning.kind == WarningKind::UnresolvedStyle
            && warning.message.contains("header story table cell (0, 0)")
            && warning.message.contains("MissingStyle")
    }));
}

#[test]
fn decode_repeated_unresolved_styles_in_header_cell_emit_summary_warning() {
    let header_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:tbl>
    <w:tr>
      <w:tc>
        <w:p>
          <w:pPr><w:pStyle w:val="MissingStyle"/></w:pPr>
          <w:r><w:t>Styled text 1</w:t></w:r>
        </w:p>
        <w:p>
          <w:pPr><w:pStyle w:val="MissingStyle"/></w:pPr>
          <w:r><w:t>Styled text 2</w:t></w:r>
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
        warning.kind == WarningKind::UnresolvedStyle
            && warning.message.contains(
                "header story table cell (0, 0) contains 2 paragraphs referencing unknown styles",
            )
    }));
}
