use super::*;

#[test]
fn decode_single_media_only_paragraph_inherits_note_and_comment_refs() {
    let body = r#"
    <w:p>
      <w:r>
        <w:drawing>
          <wp:inline>
            <wp:docPr id="1" name="AnnotatedMedia" descr="Annotated media"/>
            <a:graphic>
              <a:graphicData>
                <pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
                  <pic:blipFill>
                    <a:blip r:embed="rIdImgAnnotated"/>
                  </pic:blipFill>
                </pic:pic>
              </a:graphicData>
            </a:graphic>
          </wp:inline>
        </w:drawing>
      </w:r>
      <w:r><w:footnoteReference w:id="5"/></w:r>
      <w:r><w:commentReference w:id="7"/></w:r>
    </w:p>
    "#;
    let rels = [
        r#"<Relationship Id="rIdImgAnnotated" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/annotated.png"/>"#,
    ];
    let png_bytes: &[u8] = &[
        0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x00,
    ];

    let docx = build_docx_with_parts_and_rels(
        body,
        None,
        None,
        &rels,
        &[("word/media/annotated.png", png_bytes)],
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
        HintKind::Footnote { ref id } if id == "5"
    )));
    assert!(
        image
            .hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::Sidebar))
    );
}

#[test]
fn decode_multiple_media_only_paragraph_warns_on_ambiguous_note_attachment() {
    let body = r#"
    <w:p>
      <w:r><w:drawing><wp:inline><a:graphic><a:graphicData><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:blipFill><a:blip r:embed="rIdImg1"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>
      <w:r><w:drawing><wp:inline><a:graphic><a:graphicData><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:blipFill><a:blip r:embed="rIdImg2"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>
      <w:r><w:footnoteReference w:id="5"/></w:r>
      <w:r><w:commentReference w:id="7"/></w:r>
    </w:p>
    "#;
    let rels = [
        r#"<Relationship Id="rIdImg1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/img1.png"/>"#,
        r#"<Relationship Id="rIdImg2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/img2.png"/>"#,
    ];
    let png_bytes: &[u8] = &[
        0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x00,
    ];
    let docx = build_docx_with_parts_and_rels(
        body,
        None,
        None,
        &rels,
        &[
            ("word/media/img1.png", png_bytes),
            ("word/media/img2.png", png_bytes),
        ],
    );
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert!(result.warnings.iter().any(|warning| {
        warning.kind == WarningKind::PartialExtraction
            && warning
                .message
                .contains("media-level reference attachment is ambiguous")
    }));
}

#[test]
fn decode_mixed_text_media_paragraph_warns_on_ambiguous_note_attachment() {
    let body = r#"
    <w:p>
      <w:r><w:t>Body</w:t></w:r>
      <w:r><w:drawing><wp:inline><a:graphic><a:graphicData><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:blipFill><a:blip r:embed="rIdImg1"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>
      <w:r><w:footnoteReference w:id="5"/></w:r>
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

    assert!(result.warnings.iter().any(|warning| {
        warning.kind == WarningKind::PartialExtraction
            && warning
                .message
                .contains("media-level reference attachment is ambiguous")
    }));
}

#[test]
fn decode_single_ole_only_paragraph_inherits_note_and_comment_refs() {
    let body = r#"
    <w:p>
      <w:r><w:object><o:OLEObject xmlns:o="urn:schemas-microsoft-com:office:office"/></w:object></w:r>
      <w:r><w:footnoteReference w:id="5"/></w:r>
      <w:r><w:commentReference w:id="7"/></w:r>
    </w:p>
    "#;
    let docx = build_docx(body);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    let image = result
        .primitives
        .iter()
        .find(|primitive| matches!(primitive.kind, PrimitiveKind::Image { .. }))
        .unwrap();
    assert!(image.hints.iter().any(|hint| matches!(
        hint.kind,
        HintKind::Footnote { ref id } if id == "5"
    )));
    assert!(
        image
            .hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::Sidebar))
    );
    assert!(
        result
            .warnings
            .iter()
            .any(|warning| warning.kind == WarningKind::UnsupportedElement)
    );
}
