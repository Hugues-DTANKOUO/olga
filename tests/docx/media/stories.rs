use super::*;

#[test]
fn decode_secondary_part_images_inherit_story_hints() {
    let header_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
       xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
       xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
  <w:p>
    <w:r>
      <w:drawing>
        <wp:inline>
          <wp:docPr id="1" name="HeaderLogo" descr="Header logo"/>
          <a:graphic>
            <a:graphicData>
              <pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
                <pic:blipFill>
                  <a:blip r:embed="rIdHeaderImg"/>
                </pic:blipFill>
              </pic:pic>
            </a:graphicData>
          </a:graphic>
        </wp:inline>
      </w:drawing>
    </w:r>
  </w:p>
</w:hdr>"#;
    let footnotes_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
             xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
             xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
             xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
  <w:footnote w:id="2">
    <w:p>
      <w:r>
        <w:drawing>
          <wp:inline>
            <wp:docPr id="2" name="FootnoteIcon" descr="Footnote icon"/>
            <a:graphic>
              <a:graphicData>
                <pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
                  <pic:blipFill>
                    <a:blip r:embed="rIdFootnoteImg"/>
                  </pic:blipFill>
                </pic:pic>
              </a:graphicData>
            </a:graphic>
          </wp:inline>
        </w:drawing>
      </w:r>
    </w:p>
  </w:footnote>
</w:footnotes>"#;
    let header_rels = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdHeaderImg" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/header-logo.png"/>
</Relationships>"#;
    let footnotes_rels = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdFootnoteImg" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/footnote-icon.png"/>
</Relationships>"#;
    let png_bytes: &[u8] = &[
        0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x00,
    ];

    let docx = build_docx_with_parts_and_rels(
        r#"<w:p><w:r><w:t>Body text</w:t></w:r></w:p>"#,
        None,
        None,
        &[
            r#"<Relationship Id="rIdHeader1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/>"#,
            r#"<Relationship Id="rIdFootnotes" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/>"#,
        ],
        &[
            ("word/header1.xml", header_xml),
            ("word/_rels/header1.xml.rels", header_rels),
            ("word/footnotes.xml", footnotes_xml),
            ("word/_rels/footnotes.xml.rels", footnotes_rels),
            ("word/media/header-logo.png", png_bytes),
            ("word/media/footnote-icon.png", png_bytes),
        ],
    );

    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    let header_image = result
        .primitives
        .iter()
        .find(|primitive| {
            matches!(
                primitive.kind,
                PrimitiveKind::Image {
                    ref alt_text, ..
                } if alt_text.as_deref() == Some("Header logo")
            )
        })
        .unwrap();
    assert!(
        header_image
            .hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::PageHeader))
    );

    let footnote_image = result
        .primitives
        .iter()
        .find(|primitive| {
            matches!(
                primitive.kind,
                PrimitiveKind::Image {
                    ref alt_text, ..
                } if alt_text.as_deref() == Some("Footnote icon")
            )
        })
        .unwrap();
    assert!(footnote_image.hints.iter().any(|hint| matches!(
        hint.kind,
        HintKind::Footnote { ref id } if id == "2"
    )));
}

#[test]
fn decode_textbox_image_emits_sidebar_hint() {
    let body = r#"
    <w:txbxContent>
      <w:p>
        <w:r>
          <w:drawing>
            <wp:inline>
              <wp:docPr id="1" name="CalloutIcon" descr="Callout icon"/>
              <a:graphic>
                <a:graphicData>
                  <pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
                    <pic:blipFill>
                      <a:blip r:embed="rIdImgTextbox"/>
                    </pic:blipFill>
                  </pic:pic>
                </a:graphicData>
              </a:graphic>
            </wp:inline>
          </w:drawing>
        </w:r>
      </w:p>
    </w:txbxContent>
    "#;
    let rels = [
        r#"<Relationship Id="rIdImgTextbox" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/callout.png"/>"#,
    ];
    let png_bytes: &[u8] = &[
        0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x00,
    ];

    let docx = build_docx_with_parts_and_rels(
        body,
        None,
        None,
        &rels,
        &[("word/media/callout.png", png_bytes)],
    );
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    let image = result
        .primitives
        .iter()
        .find(|primitive| {
            matches!(
                primitive.kind,
                PrimitiveKind::Image {
                    ref alt_text, ..
                } if alt_text.as_deref() == Some("Callout icon")
            )
        })
        .unwrap();
    assert!(
        image
            .hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::Sidebar))
    );
}
