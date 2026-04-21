use olga::error::WarningKind;
use olga::formats::docx::DocxDecoder;
use olga::model::*;
use olga::traits::FormatDecoder;

use crate::support::docx::{build_docx, build_docx_with_parts, build_docx_with_parts_and_rels};

#[test]
fn decode_simple_paragraphs() {
    let body = r#"
    <w:p>
      <w:r><w:t>First paragraph</w:t></w:r>
    </w:p>
    <w:p>
      <w:r><w:t>Second paragraph</w:t></w:r>
    </w:p>
    "#;

    let docx = build_docx(body);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert_eq!(result.primitives.len(), 2);
    assert_eq!(result.primitives[0].text_content(), Some("First paragraph"));
    assert_eq!(
        result.primitives[1].text_content(),
        Some("Second paragraph")
    );

    assert!(
        result.primitives[0]
            .hints
            .iter()
            .any(|h| matches!(h.kind, HintKind::Paragraph))
    );
    assert!(
        result.primitives[1]
            .hints
            .iter()
            .any(|h| matches!(h.kind, HintKind::Paragraph))
    );
}

#[test]
fn decode_headings() {
    let styles = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="Heading1">
    <w:name w:val="Heading 1"/>
    <w:pPr><w:outlineLvl w:val="0"/></w:pPr>
    <w:rPr><w:b/><w:sz w:val="32"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading2">
    <w:name w:val="Heading 2"/>
    <w:basedOn w:val="Heading1"/>
    <w:pPr><w:outlineLvl w:val="1"/></w:pPr>
    <w:rPr><w:sz w:val="26"/></w:rPr>
  </w:style>
</w:styles>"#;

    let body = r#"
    <w:p>
      <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
      <w:r><w:t>Chapter One</w:t></w:r>
    </w:p>
    <w:p>
      <w:pPr><w:pStyle w:val="Heading2"/></w:pPr>
      <w:r><w:t>Section 1.1</w:t></w:r>
    </w:p>
    <w:p>
      <w:r><w:t>Body text</w:t></w:r>
    </w:p>
    "#;

    let docx = build_docx_with_parts(body, Some(styles), None, &[]);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert_eq!(result.primitives.len(), 3);
    assert!(
        result.primitives[0]
            .hints
            .iter()
            .any(|h| matches!(h.kind, HintKind::Heading { level: 1 }))
    );
    assert!(
        result.primitives[1]
            .hints
            .iter()
            .any(|h| matches!(h.kind, HintKind::Heading { level: 2 }))
    );
    assert!(
        result.primitives[2]
            .hints
            .iter()
            .any(|h| matches!(h.kind, HintKind::Paragraph))
    );
    assert!(
        !result
            .warnings
            .iter()
            .any(|warning| warning.kind == WarningKind::HeuristicInference)
    );
}

#[test]
fn decode_heading_style_name_inference_warns() {
    let styles = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="HeadingNameFallback">
    <w:name w:val="Heading 3"/>
    <w:rPr><w:b/><w:sz w:val="28"/></w:rPr>
  </w:style>
</w:styles>"#;

    let body = r#"
    <w:p>
      <w:pPr><w:pStyle w:val="HeadingNameFallback"/></w:pPr>
      <w:r><w:t>Fallback heading</w:t></w:r>
    </w:p>
    "#;

    let docx = build_docx_with_parts(body, Some(styles), None, &[]);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert_eq!(result.primitives.len(), 1);
    assert!(
        result.primitives[0]
            .hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::Heading { level: 3 }))
    );
    assert!(result.warnings.iter().any(|warning| {
        warning.kind == WarningKind::HeuristicInference
            && warning.message.contains("HeadingNameFallback")
            && warning.message.contains("outlineLvl is absent")
    }));
}

#[test]
fn decode_repeated_heading_style_name_inference_in_header_cell_emit_summary_warning() {
    let styles = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="HeadingNameFallback">
    <w:name w:val="Heading 3"/>
    <w:rPr><w:b/><w:sz w:val="28"/></w:rPr>
  </w:style>
</w:styles>"#;

    let header_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:tbl>
    <w:tr>
      <w:tc>
        <w:p>
          <w:pPr><w:pStyle w:val="HeadingNameFallback"/></w:pPr>
          <w:r><w:t>Fallback heading 1</w:t></w:r>
        </w:p>
        <w:p>
          <w:pPr><w:pStyle w:val="HeadingNameFallback"/></w:pPr>
          <w:r><w:t>Fallback heading 2</w:t></w:r>
        </w:p>
      </w:tc>
    </w:tr>
  </w:tbl>
</w:hdr>"#;

    let docx = build_docx_with_parts_and_rels(
        r#"<w:p><w:r><w:t>Body text</w:t></w:r></w:p>"#,
        Some(styles),
        None,
        &[
            r#"<Relationship Id="rIdHeader1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/>"#,
        ],
        &[("word/header1.xml", header_xml)],
    );
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert!(result.warnings.iter().any(|warning| {
        warning.kind == WarningKind::HeuristicInference
            && warning.message.contains(
                "header story table cell (0, 0) contains 2 paragraphs promoted to headings via style-name heuristic",
            )
    }));
}

#[test]
fn decode_list_items() {
    let numbering = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="0">
    <w:lvl w:ilvl="0"><w:numFmt w:val="bullet"/></w:lvl>
    <w:lvl w:ilvl="1"><w:numFmt w:val="bullet"/></w:lvl>
  </w:abstractNum>
  <w:abstractNum w:abstractNumId="1">
    <w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>
  <w:num w:numId="2"><w:abstractNumId w:val="1"/></w:num>
</w:numbering>"#;

    let body = r#"
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="0"/></w:numPr></w:pPr>
      <w:r><w:t>Bullet item 1</w:t></w:r>
    </w:p>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="1"/></w:numPr></w:pPr>
      <w:r><w:t>Nested bullet</w:t></w:r>
    </w:p>
    <w:p>
      <w:pPr><w:numPr><w:numId w:val="2"/><w:ilvl w:val="0"/></w:numPr></w:pPr>
      <w:r><w:t>Numbered item 1</w:t></w:r>
    </w:p>
    "#;

    let docx = build_docx_with_parts(body, None, Some(numbering), &[]);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert_eq!(result.primitives.len(), 3);
    assert!(result.primitives[0].hints.iter().any(|h| matches!(
        h.kind,
        HintKind::ListItem {
            depth: 0,
            ordered: false,
            ..
        }
    )));
    assert!(result.primitives[1].hints.iter().any(|h| matches!(
        h.kind,
        HintKind::ListItem {
            depth: 1,
            ordered: false,
            ..
        }
    )));
    assert!(result.primitives[2].hints.iter().any(|h| matches!(
        h.kind,
        HintKind::ListItem {
            depth: 0,
            ordered: true,
            ..
        }
    )));
}

#[test]
fn decode_multiple_runs() {
    let body = r#"
    <w:p>
      <w:r><w:t>Hello </w:t></w:r>
      <w:r><w:rPr><w:b/></w:rPr><w:t>bold</w:t></w:r>
      <w:r><w:t> world</w:t></w:r>
    </w:p>
    "#;

    let docx = build_docx(body);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert_eq!(result.primitives.len(), 1);
    assert_eq!(
        result.primitives[0].text_content(),
        Some("Hello bold world")
    );
}

#[test]
fn decode_page_break() {
    let body = r#"
    <w:p>
      <w:r><w:t>Page 1 content</w:t></w:r>
    </w:p>
    <w:p>
      <w:r><w:br w:type="page"/></w:r>
    </w:p>
    <w:p>
      <w:r><w:t>Page 2 content</w:t></w:r>
    </w:p>
    "#;

    let docx = build_docx(body);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    let p1 = result
        .primitives
        .iter()
        .find(|p| p.text_content() == Some("Page 1 content"))
        .unwrap();
    assert_eq!(p1.page, 0);

    let p2 = result
        .primitives
        .iter()
        .find(|p| p.text_content() == Some("Page 2 content"))
        .unwrap();
    assert_eq!(p2.page, 1);
}

#[test]
fn decode_tracked_changes() {
    let body = r#"
    <w:p>
      <w:r><w:t>Keep this</w:t></w:r>
      <w:del w:id="1" w:author="Test">
        <w:r><w:t> delete this</w:t></w:r>
      </w:del>
      <w:ins w:id="2" w:author="Test">
        <w:r><w:t> and add this</w:t></w:r>
      </w:ins>
    </w:p>
    "#;

    let docx = build_docx(body);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert_eq!(result.primitives.len(), 1);
    assert_eq!(
        result.primitives[0].text_content(),
        Some("Keep this and add this")
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.kind == WarningKind::TrackedChangeResolved)
    );
}

#[test]
fn decode_sdt_content() {
    let body = r#"
    <w:sdt>
      <w:sdtPr><w:alias w:val="Title"/></w:sdtPr>
      <w:sdtContent>
        <w:p>
          <w:r><w:t>Content inside SDT</w:t></w:r>
        </w:p>
      </w:sdtContent>
    </w:sdt>
    "#;

    let docx = build_docx(body);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert_eq!(result.primitives.len(), 1);
    assert_eq!(
        result.primitives[0].text_content(),
        Some("Content inside SDT")
    );
}

#[test]
fn decode_alternate_content_prefers_fallback() {
    let body = r#"
    <mc:AlternateContent>
      <mc:Choice Requires="wps">
        <w:p><w:r><w:t>Choice content (should be skipped)</w:t></w:r></w:p>
      </mc:Choice>
      <mc:Fallback>
        <w:p><w:r><w:t>Fallback content</w:t></w:r></w:p>
      </mc:Fallback>
    </mc:AlternateContent>
    "#;

    let docx = build_docx(body);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert_eq!(result.primitives.len(), 1);
    assert_eq!(
        result.primitives[0].text_content(),
        Some("Fallback content")
    );
}

#[test]
fn decode_bidi_paragraph() {
    let body = r#"
    <w:p>
      <w:pPr><w:bidi/></w:pPr>
      <w:r><w:t>مرحبا</w:t></w:r>
    </w:p>
    "#;

    let docx = build_docx(body);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert_eq!(result.primitives.len(), 1);
    match &result.primitives[0].kind {
        PrimitiveKind::Text { text_direction, .. } => {
            assert_eq!(*text_direction, TextDirection::RightToLeft);
        }
        _ => panic!("Expected Text primitive"),
    }
}
