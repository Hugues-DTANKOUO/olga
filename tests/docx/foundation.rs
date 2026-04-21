use olga::formats::docx::DocxDecoder;
use olga::model::*;
use olga::traits::FormatDecoder;

use crate::support::docx::{build_docx, build_docx_with_parts, build_docx_with_parts_and_rels};

#[test]
fn decode_empty_paragraph_skipped() {
    let body = r#"
    <w:p></w:p>
    <w:p><w:r><w:t>Not empty</w:t></w:r></w:p>
    "#;

    let docx = build_docx(body);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert_eq!(result.primitives.len(), 1);
    assert_eq!(result.primitives[0].text_content(), Some("Not empty"));
}

#[test]
fn decode_source_order_monotonic() {
    let body = r#"
    <w:p><w:r><w:t>First</w:t></w:r></w:p>
    <w:p><w:r><w:t>Second</w:t></w:r></w:p>
    <w:p><w:r><w:t>Third</w:t></w:r></w:p>
    "#;

    let docx = build_docx(body);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    for i in 1..result.primitives.len() {
        assert!(result.primitives[i].source_order > result.primitives[i - 1].source_order);
    }
}

#[test]
fn decode_style_inheritance_bold() {
    let styles = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="Normal">
    <w:name w:val="Normal"/>
    <w:rPr><w:sz w:val="22"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="BoldStyle">
    <w:name w:val="Bold Style"/>
    <w:basedOn w:val="Normal"/>
    <w:rPr><w:b/></w:rPr>
  </w:style>
</w:styles>"#;

    let body = r#"
    <w:p>
      <w:pPr><w:pStyle w:val="BoldStyle"/></w:pPr>
      <w:r><w:t>Should be bold</w:t></w:r>
    </w:p>
    "#;

    let docx = build_docx_with_parts(body, Some(styles), None, &[]);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    match &result.primitives[0].kind {
        PrimitiveKind::Text { is_bold, .. } => assert!(*is_bold),
        _ => panic!("Expected Text"),
    }
}

#[test]
fn decode_lang_from_run() {
    let body = r#"
    <w:p>
      <w:r>
        <w:rPr><w:lang w:val="fr-FR"/></w:rPr>
        <w:t>Bonjour le monde</w:t>
      </w:r>
    </w:p>
    "#;

    let docx = build_docx(body);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert_eq!(result.primitives.len(), 1);
    match &result.primitives[0].kind {
        PrimitiveKind::Text { lang, .. } => assert_eq!(lang.as_deref(), Some("fr-FR")),
        _ => panic!("Expected Text"),
    }
}

#[test]
fn decode_lang_bidi_fallback() {
    let body = r#"
    <w:p>
      <w:pPr><w:bidi/></w:pPr>
      <w:r>
        <w:rPr><w:lang w:bidi="ar-SA"/></w:rPr>
        <w:t>مرحبا</w:t>
      </w:r>
    </w:p>
    "#;

    let docx = build_docx(body);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert_eq!(result.primitives.len(), 1);
    match &result.primitives[0].kind {
        PrimitiveKind::Text {
            lang,
            text_direction,
            ..
        } => {
            assert_eq!(lang.as_deref(), Some("ar-SA"));
            assert_eq!(*text_direction, TextDirection::RightToLeft);
        }
        _ => panic!("Expected Text"),
    }
}

#[test]
fn decode_lang_none_when_absent() {
    let body = r#"
    <w:p>
      <w:r><w:t>No language tag</w:t></w:r>
    </w:p>
    "#;

    let docx = build_docx(body);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    match &result.primitives[0].kind {
        PrimitiveKind::Text { lang, .. } => assert!(lang.is_none()),
        _ => panic!("Expected Text"),
    }
}

#[test]
fn decode_lang_mixed_runs_uses_first() {
    let body = r#"
    <w:p>
      <w:r>
        <w:rPr><w:lang w:val="en-US"/></w:rPr>
        <w:t>Hello </w:t>
      </w:r>
      <w:r>
        <w:rPr><w:lang w:val="fr-FR"/></w:rPr>
        <w:t>monde</w:t>
      </w:r>
    </w:p>
    "#;

    let docx = build_docx(body);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    match &result.primitives[0].kind {
        PrimitiveKind::Text { lang, content, .. } => {
            assert_eq!(content, "Hello monde");
            assert_eq!(lang.as_deref(), Some("en-US"));
        }
        _ => panic!("Expected Text"),
    }
}

#[test]
fn decode_secondary_parts_emit_story_hints() {
    let header_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:p><w:r><w:t>Header text</w:t></w:r></w:p>
</w:hdr>"#;
    let footer_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:p><w:r><w:t>Footer text</w:t></w:r></w:p>
</w:ftr>"#;
    let footnotes_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:footnote w:id="2"><w:p><w:r><w:t>Footnote body</w:t></w:r></w:p></w:footnote>
</w:footnotes>"#;
    let endnotes_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:endnote w:id="7"><w:p><w:r><w:t>Endnote body</w:t></w:r></w:p></w:endnote>
</w:endnotes>"#;
    let comments_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:comment w:id="9"><w:p><w:r><w:t>Comment body</w:t></w:r></w:p></w:comment>
</w:comments>"#;

    let docx = build_docx_with_parts_and_rels(
        r#"<w:p><w:r><w:t>Body text</w:t></w:r></w:p>"#,
        None,
        None,
        &[
            r#"<Relationship Id="rIdHeader1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/>"#,
            r#"<Relationship Id="rIdFooter1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/>"#,
            r#"<Relationship Id="rIdFootnotes" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/>"#,
            r#"<Relationship Id="rIdEndnotes" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes" Target="endnotes.xml"/>"#,
            r#"<Relationship Id="rIdComments" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/>"#,
        ],
        &[
            ("word/header1.xml", header_xml),
            ("word/footer1.xml", footer_xml),
            ("word/footnotes.xml", footnotes_xml),
            ("word/endnotes.xml", endnotes_xml),
            ("word/comments.xml", comments_xml),
        ],
    );

    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    let header = result
        .primitives
        .iter()
        .find(|prim| prim.text_content() == Some("Header text"))
        .unwrap();
    assert!(
        header
            .hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::PageHeader))
    );

    let footer = result
        .primitives
        .iter()
        .find(|prim| prim.text_content() == Some("Footer text"))
        .unwrap();
    assert!(
        footer
            .hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::PageFooter))
    );

    let footnote = result
        .primitives
        .iter()
        .find(|prim| prim.text_content() == Some("Footnote body"))
        .unwrap();
    assert!(footnote.hints.iter().any(|hint| matches!(
        hint.kind,
        HintKind::Footnote { ref id } if id == "2"
    )));

    let endnote = result
        .primitives
        .iter()
        .find(|prim| prim.text_content() == Some("Endnote body"))
        .unwrap();
    assert!(endnote.hints.iter().any(|hint| matches!(
        hint.kind,
        HintKind::Footnote { ref id } if id == "7"
    )));

    let comment = result
        .primitives
        .iter()
        .find(|prim| prim.text_content() == Some("Comment body"))
        .unwrap();
    assert!(
        comment
            .hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::Sidebar))
    );
}

#[test]
fn decode_footnote_reference_attaches_id_hint() {
    let body = r#"
    <w:p>
      <w:r><w:t>Paragraph with note</w:t></w:r>
      <w:r><w:footnoteReference w:id="5"/></w:r>
    </w:p>
    "#;

    let docx = build_docx(body);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert_eq!(result.primitives.len(), 1);
    assert!(result.primitives[0].hints.iter().any(|hint| matches!(
        hint.kind,
        HintKind::Footnote { ref id } if id == "5"
    )));
}

#[test]
fn decode_textbox_paragraphs_emit_sidebar_hints() {
    let body = r#"
    <w:txbxContent>
      <w:p>
        <w:r><w:t>Sidebar callout</w:t></w:r>
      </w:p>
    </w:txbxContent>
    "#;

    let docx = build_docx(body);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    let prim = result
        .primitives
        .iter()
        .find(|prim| prim.text_content() == Some("Sidebar callout"))
        .unwrap();
    assert!(
        prim.hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::Sidebar))
    );
}

#[test]
fn decoder_trait_basics() {
    let decoder = DocxDecoder::new();
    assert_eq!(decoder.name(), "DOCX");
    assert!(decoder.supported_extensions().contains(&"docx"));
    assert!(decoder.supported_extensions().contains(&"docm"));
}
