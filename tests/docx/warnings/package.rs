use olga::error::WarningKind;
use olga::formats::docx::DocxDecoder;
use olga::traits::FormatDecoder;

use crate::support::docx::{build_docx_with_parts, build_docx_with_parts_and_rels};

#[test]
fn decode_missing_styles_warns() {
    let body = r#"
    <w:p>
      <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
      <w:r><w:t>Styled text</w:t></w:r>
    </w:p>
    "#;

    let docx = build_docx_with_parts(body, None, None, &[]);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.kind == WarningKind::MissingPart)
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.kind == WarningKind::UnresolvedStyle)
    );
}

#[test]
fn decode_repeated_missing_secondary_header_parts_emit_summary_warning() {
    let body = r#"<w:p><w:r><w:t>Body text</w:t></w:r></w:p>"#;
    let rels = [
        r#"<Relationship Id="rIdHeader1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/>"#,
        r#"<Relationship Id="rIdHeader2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header2.xml"/>"#,
    ];

    let docx = build_docx_with_parts_and_rels(body, None, None, &rels, &[]);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert!(result.warnings.iter().any(|warning| {
        warning.kind == WarningKind::MissingPart
            && warning.message.contains("2 missing header parts")
    }));
}

#[test]
fn decode_repeated_missing_media_payloads_emit_package_summary_warning() {
    let body = r#"<w:p><w:r><w:t>Body text</w:t></w:r></w:p>"#;
    let rels = [
        r#"<Relationship Id="rIdImg1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>"#,
        r#"<Relationship Id="rIdImg2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image2.png"/>"#,
    ];

    let docx = build_docx_with_parts_and_rels(body, None, None, &rels, &[]);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert!(result.warnings.iter().any(|warning| {
        warning.kind == WarningKind::MissingMedia
            && warning.message.contains(
                "DOCX package contains 2 missing media payloads referenced by relationship parts",
            )
    }));
}

#[test]
fn decode_repeated_malformed_secondary_relationship_parts_emit_package_summary_warning() {
    let body = r#"<w:p><w:r><w:t>Body text</w:t></w:r></w:p>"#;
    let rels = [
        r#"<Relationship Id="rIdHeader1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/>"#,
        r#"<Relationship Id="rIdHeader2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header2.xml"/>"#,
    ];
    let header_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:p><w:r><w:t>Header</w:t></w:r></w:p>
</w:hdr>"#;
    let bad_rels = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1""#;

    let docx = build_docx_with_parts_and_rels(
        body,
        None,
        None,
        &rels,
        &[
            ("word/header1.xml", header_xml),
            ("word/header2.xml", header_xml),
            ("word/_rels/header1.xml.rels", bad_rels),
            ("word/_rels/header2.xml.rels", bad_rels),
        ],
    );
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert!(result.warnings.iter().any(|warning| {
        warning.kind == WarningKind::MalformedContent
            && warning
                .message
                .contains("DOCX package contains 2 malformed relationship parts under word/_rels/")
    }));
}

#[test]
fn decode_repeated_malformed_secondary_story_xml_parts_emit_package_summary_warning() {
    let body = r#"<w:p><w:r><w:t>Body text</w:t></w:r></w:p>"#;
    let rels = [
        r#"<Relationship Id="rIdHeader1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/>"#,
        r#"<Relationship Id="rIdHeader2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header2.xml"/>"#,
    ];
    let bad_header = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:p><w:r><w:t>Broken header</w:t></w:r></w:p>
  <"#;

    let docx = build_docx_with_parts_and_rels(
        body,
        None,
        None,
        &rels,
        &[
            ("word/header1.xml", bad_header),
            ("word/header2.xml", bad_header),
        ],
    );
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert!(result.warnings.iter().any(|warning| {
        warning.kind == WarningKind::UnexpectedStructure
            && warning.message.contains(
                "DOCX package contains 2 malformed secondary story XML parts; inspect story-level XML parse warnings for detail",
            )
    }));
}

#[test]
fn decode_repeated_broken_numbering_definitions_emit_package_summary_warning() {
    let numbering = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
    <w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
      <w:num w:numId="1"><w:abstractNumId w:val="42"/></w:num>
      <w:num w:numId="2"><w:abstractNumId w:val="43"/></w:num>
    </w:numbering>
    "#;

    let docx = build_docx_with_parts(
        r#"<w:p><w:r><w:t>Body text</w:t></w:r></w:p>"#,
        None,
        Some(numbering),
        &[],
    );
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert!(result.warnings.iter().any(|warning| {
        warning.kind == WarningKind::PartialExtraction
            && warning.message.contains(
                "DOCX numbering.xml contains 2 numbering definitions referencing missing abstract numbering entries",
            )
    }));
}

#[test]
fn decode_repeated_unresolved_style_inheritance_emit_package_summary_warning() {
    let styles = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="BrokenStyle1">
    <w:name w:val="Broken Style 1"/>
    <w:basedOn w:val="MissingBase1"/>
  </w:style>
  <w:style w:type="paragraph" w:styleId="BrokenStyle2">
    <w:name w:val="Broken Style 2"/>
    <w:basedOn w:val="MissingBase2"/>
  </w:style>
</w:styles>"#;

    let body = r#"
    <w:p>
      <w:pPr><w:pStyle w:val="BrokenStyle1"/></w:pPr>
      <w:r><w:t>Broken style 1</w:t></w:r>
    </w:p>
    <w:p>
      <w:pPr><w:pStyle w:val="BrokenStyle2"/></w:pPr>
      <w:r><w:t>Broken style 2</w:t></w:r>
    </w:p>
    "#;

    let docx = build_docx_with_parts(body, Some(styles), None, &[]);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert!(result.warnings.iter().any(|warning| {
        warning.kind == WarningKind::UnresolvedStyle
            && warning.message.contains(
                "DOCX styles.xml contains 2 unresolved style references in inheritance chains",
            )
    }));
}

#[test]
fn decode_repeated_malformed_auxiliary_xml_files_emit_package_summary_warning() {
    let styles = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="Broken""#;
    let numbering = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="0""#;

    let docx = build_docx_with_parts(
        r#"<w:p><w:r><w:t>Body text</w:t></w:r></w:p>"#,
        Some(styles),
        Some(numbering),
        &[],
    );
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert!(result.warnings.iter().any(|warning| {
        warning.kind == WarningKind::MalformedContent
            && warning.message.contains(
                "DOCX package contains 2 malformed auxiliary XML files among styles.xml, numbering.xml, or docProps/app.xml",
            )
    }));
}
