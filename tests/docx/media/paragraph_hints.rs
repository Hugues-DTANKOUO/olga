use super::*;

#[test]
fn decode_media_in_heading_paragraph_inherits_heading_hint() {
    let body = r#"
    <w:p>
      <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
      <w:r><w:drawing><wp:inline><a:graphic><a:graphicData><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:blipFill><a:blip r:embed="rIdImg1"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>
    </w:p>
    "#;
    let styles = r#"<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="Heading 1"/><w:outlineLvl w:val="0"/></w:style>"#;
    let rels = [
        r#"<Relationship Id="rIdImg1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/img1.png"/>"#,
    ];
    let png_bytes: &[u8] = &[
        0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x00,
    ];
    let docx = build_docx_with_parts_and_rels(
        body,
        Some(styles),
        None,
        &rels,
        &[("word/media/img1.png", png_bytes)],
    );

    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();
    let image = result
        .primitives
        .iter()
        .find(|primitive| matches!(primitive.kind, PrimitiveKind::Image { .. }))
        .unwrap();

    assert!(
        image
            .hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::Heading { level: 1 }))
    );
}

#[test]
fn decode_media_in_list_item_paragraph_inherits_list_hint() {
    let body = r#"
    <w:p>
      <w:pPr>
        <w:numPr>
          <w:ilvl w:val="1"/>
          <w:numId w:val="42"/>
        </w:numPr>
      </w:pPr>
      <w:r><w:drawing><wp:inline><a:graphic><a:graphicData><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:blipFill><a:blip r:embed="rIdImg1"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>
    </w:p>
    "#;
    let numbering = r#"
      <w:abstractNum w:abstractNumId="7">
        <w:lvl w:ilvl="1"><w:numFmt w:val="bullet"/></w:lvl>
      </w:abstractNum>
      <w:num w:numId="42"><w:abstractNumId w:val="7"/></w:num>
    "#;
    let rels = [
        r#"<Relationship Id="rIdImg1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/img1.png"/>"#,
    ];
    let png_bytes: &[u8] = &[
        0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x00,
    ];
    let docx = build_docx_with_parts_and_rels(
        body,
        None,
        Some(numbering),
        &rels,
        &[("word/media/img1.png", png_bytes)],
    );

    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();
    let image = result
        .primitives
        .iter()
        .find(|primitive| matches!(primitive.kind, PrimitiveKind::Image { .. }))
        .unwrap();

    assert!(image.hints.iter().any(|hint| matches!(
        hint.kind,
        HintKind::ListItem {
            depth: 1,
            ordered: false,
            ..
        }
    )));
}

#[test]
fn decode_media_in_plain_paragraph_inherits_paragraph_hint() {
    let body = r#"
    <w:p>
      <w:r><w:drawing><wp:inline><a:graphic><a:graphicData><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:blipFill><a:blip r:embed="rIdImg1"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>
    </w:p>
    "#;
    let rels = [
        r#"<Relationship Id="rIdImg1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/img1.png"/>"#,
    ];
    let png_bytes: &[u8] = &[
        0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x00,
    ];
    let docx = build_docx_with_parts_and_rels(
        body,
        None,
        None,
        &rels,
        &[("word/media/img1.png", png_bytes)],
    );

    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();
    let image = result
        .primitives
        .iter()
        .find(|primitive| matches!(primitive.kind, PrimitiveKind::Image { .. }))
        .unwrap();

    assert!(
        image
            .hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::Paragraph))
    );
}
