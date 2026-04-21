use super::*;

#[test]
fn decode_repeated_media_annotation_ambiguity_in_same_cell_emits_summary_warning() {
    let body = r#"
    <w:tbl>
      <w:tr>
        <w:tc>
          <w:p><w:r><w:t>A</w:t></w:r><w:r><w:drawing><a:graphic><a:graphicData><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:blipFill><a:blip r:embed="rIdImg1"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></w:drawing></w:r><w:r><w:footnoteReference w:id="5"/></w:r></w:p>
          <w:p><w:r><w:t>B</w:t></w:r><w:r><w:drawing><a:graphic><a:graphicData><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:blipFill><a:blip r:embed="rIdImg2"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></w:drawing></w:r><w:r><w:commentReference w:id="7"/></w:r></w:p>
        </w:tc>
      </w:tr>
    </w:tbl>
    "#;
    let rels = [
        r#"<Relationship Id="rIdImg1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/img1.png"/>"#,
        r#"<Relationship Id="rIdImg2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/img2.png"/>"#,
    ];
    let png_bytes: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
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
                .contains("table cell (0, 0) contains 2 paragraphs")
    }));
}

#[test]
fn decode_repeated_media_annotation_ambiguity_in_comment_story_emits_story_id_summary_warning() {
    let comments_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:comment w:id="9">
    <w:p><w:r><w:t>A</w:t></w:r><w:r><w:drawing><a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:blipFill><a:blip r:embed="rIdImg1"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></w:drawing></w:r><w:r><w:commentReference w:id="9"/></w:r></w:p>
    <w:p><w:r><w:t>B</w:t></w:r><w:r><w:drawing><a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:blipFill><a:blip r:embed="rIdImg2"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></w:drawing></w:r><w:r><w:commentReference w:id="9"/></w:r></w:p>
  </w:comment>
</w:comments>"#;
    let comments_rels = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdImg1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/img1.png"/>
  <Relationship Id="rIdImg2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/img2.png"/>
</Relationships>"#;
    let png_bytes: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let docx = build_docx_with_parts_and_rels(
        r#"<w:p><w:r><w:t>Body</w:t></w:r></w:p>"#,
        None,
        None,
        &[
            r#"<Relationship Id="rIdComments" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/>"#,
        ],
        &[
            ("word/comments.xml", comments_xml),
            ("word/_rels/comments.xml.rels", comments_rels),
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
                .contains("comment story 9 contains 2 paragraphs")
    }));
}

#[test]
fn decode_repeated_media_annotation_ambiguity_in_body_flow_emits_summary_warning() {
    let body = r#"
    <w:p><w:r><w:t>A</w:t></w:r><w:r><w:drawing><a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:blipFill><a:blip r:embed="rIdImg1"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></w:drawing></w:r><w:r><w:footnoteReference w:id="5"/></w:r></w:p>
    <w:p><w:r><w:t>B</w:t></w:r><w:r><w:drawing><a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:blipFill><a:blip r:embed="rIdImg2"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></w:drawing></w:r><w:r><w:commentReference w:id="7"/></w:r></w:p>
    "#;
    let rels = [
        r#"<Relationship Id="rIdImg1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/img1.png"/>"#,
        r#"<Relationship Id="rIdImg2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/img2.png"/>"#,
    ];
    let png_bytes: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
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
                .contains("body flow on page 0 contains 2 paragraphs")
    }));
}

#[test]
fn decode_repeated_media_annotation_ambiguity_in_header_table_cell_emits_scoped_summary_warning() {
    let header_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:tbl><w:tr><w:tc>
    <w:p><w:r><w:t>A</w:t></w:r><w:r><w:drawing><a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:blipFill><a:blip r:embed="rIdImg1"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></w:drawing></w:r><w:r><w:footnoteReference w:id="5"/></w:r></w:p>
    <w:p><w:r><w:t>B</w:t></w:r><w:r><w:drawing><a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:blipFill><a:blip r:embed="rIdImg2"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></w:drawing></w:r><w:r><w:commentReference w:id="7"/></w:r></w:p>
  </w:tc></w:tr></w:tbl>
</w:hdr>"#;
    let header_rels = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdImg1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/img1.png"/>
  <Relationship Id="rIdImg2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/img2.png"/>
</Relationships>"#;
    let png_bytes: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let docx = build_docx_with_parts_and_rels(
        r#"<w:p><w:r><w:t>Body</w:t></w:r></w:p>"#,
        None,
        None,
        &[
            r#"<Relationship Id="rIdHeader1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/>"#,
        ],
        &[
            ("word/header1.xml", header_xml),
            ("word/_rels/header1.xml.rels", header_rels),
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
                .contains("header story table cell (0, 0) contains 2 paragraphs")
    }));
}
