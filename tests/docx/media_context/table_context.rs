use super::*;

#[test]
fn decode_table_cell_images_inherit_table_cell_hints() {
    let body = r#"
    <w:tbl>
      <w:tr>
        <w:tc>
          <w:p>
            <w:r>
              <w:drawing>
                <a:graphic>
                  <a:graphicData>
                    <pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
                      <pic:blipFill>
                        <a:blip r:embed="rIdImg1"/>
                      </pic:blipFill>
                    </pic:pic>
                  </a:graphicData>
                </a:graphic>
              </w:drawing>
            </w:r>
          </w:p>
        </w:tc>
        <w:tc>
          <w:p><w:r><w:t>Neighbor</w:t></w:r></w:p>
        </w:tc>
      </w:tr>
    </w:tbl>
    "#;

    let rels = [
        r#"<Relationship Id="rIdImg1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>"#,
    ];
    let png_bytes: &[u8] = &[
        0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x00,
    ];
    let extra_parts = [("word/media/image1.png", png_bytes)];

    let docx = build_docx_with_parts_and_rels(body, None, None, &rels, &extra_parts);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    let image = result
        .primitives
        .iter()
        .find(|primitive| matches!(primitive.kind, PrimitiveKind::Image { .. }))
        .unwrap();
    let neighbor = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Neighbor"))
        .unwrap();

    assert!(image.hints.iter().any(|hint| {
        matches!(
            hint.kind,
            HintKind::TableCell {
                row: 0,
                col: 0,
                rowspan: 1,
                colspan: 1,
            }
        )
    }));
    assert!(neighbor.hints.iter().any(|hint| {
        matches!(
            hint.kind,
            HintKind::TableCell {
                row: 0,
                col: 1,
                rowspan: 1,
                colspan: 1,
            }
        )
    }));
    match &image.kind {
        PrimitiveKind::Image { format, .. } => assert_eq!(*format, ImageFormat::Png),
        _ => panic!("Expected image primitive"),
    }
}

#[test]
fn decode_table_cell_hyperlinked_image_preserves_alt_text_and_link_hint() {
    let body = r#"
    <w:tbl>
      <w:tr>
        <w:tc>
          <w:p>
            <w:hyperlink r:id="rIdLinkImg">
              <w:r>
                <w:drawing>
                  <wp:inline>
                    <wp:docPr id="1" name="Logo" descr="Company logo"/>
                    <a:graphic>
                      <a:graphicData>
                        <pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
                          <pic:blipFill>
                            <a:blip r:embed="rIdImg1"/>
                          </pic:blipFill>
                        </pic:pic>
                      </a:graphicData>
                    </a:graphic>
                  </wp:inline>
                </w:drawing>
              </w:r>
            </w:hyperlink>
          </w:p>
        </w:tc>
      </w:tr>
    </w:tbl>
    "#;

    let rels = [
        r#"<Relationship Id="rIdImg1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>"#,
        r#"<Relationship Id="rIdLinkImg" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/logo" TargetMode="External"/>"#,
    ];
    let png_bytes: &[u8] = &[
        0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x00,
    ];
    let extra_parts = [("word/media/image1.png", png_bytes)];

    let docx = build_docx_with_parts_and_rels(body, None, None, &rels, &extra_parts);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    let image = result
        .primitives
        .iter()
        .find(|primitive| matches!(primitive.kind, PrimitiveKind::Image { .. }))
        .unwrap();

    assert!(image.hints.iter().any(|hint| {
        matches!(
            hint.kind,
            HintKind::TableCell {
                row: 0,
                col: 0,
                rowspan: 1,
                colspan: 1,
            }
        )
    }));
    assert!(image.hints.iter().any(|hint| {
        matches!(
            hint.kind,
            HintKind::Link { ref url } if url == "https://example.com/logo"
        )
    }));
    match &image.kind {
        PrimitiveKind::Image {
            format, alt_text, ..
        } => {
            assert_eq!(*format, ImageFormat::Png);
            assert_eq!(alt_text.as_deref(), Some("Company logo"));
        }
        _ => panic!("Expected image primitive"),
    }
}

#[test]
fn decode_media_inside_nested_table_inherits_flattened_container_hint() {
    let body = r#"
    <w:tbl>
      <w:tr>
        <w:tc>
          <w:tbl>
            <w:tr>
              <w:tc>
                <w:p>
                  <w:r>
                    <w:drawing>
                      <wp:inline>
                        <wp:docPr id="1" name="NestedMedia" descr="Nested media"/>
                        <a:graphic>
                          <a:graphicData>
                            <pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
                              <pic:blipFill>
                                <a:blip r:embed="rIdNestedImg"/>
                              </pic:blipFill>
                            </pic:pic>
                          </a:graphicData>
                        </a:graphic>
                      </wp:inline>
                    </w:drawing>
                  </w:r>
                </w:p>
              </w:tc>
            </w:tr>
          </w:tbl>
        </w:tc>
      </w:tr>
    </w:tbl>
    "#;
    let rels = [
        r#"<Relationship Id="rIdNestedImg" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/nested.png"/>"#,
    ];
    let png_bytes: &[u8] = &[
        0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x00,
    ];

    let docx = build_docx_with_parts_and_rels(
        body,
        None,
        None,
        &rels,
        &[("word/media/nested.png", png_bytes)],
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
                } if alt_text.as_deref() == Some("Nested media")
            )
        })
        .unwrap();
    assert!(image.hints.iter().any(|hint| matches!(
        hint.kind,
        HintKind::FlattenedFromTableCell {
            row: 0,
            col: 0,
            depth: 1
        }
    )));
}
