use super::*;

#[test]
fn decode_repeated_note_paragraphs_without_id_emit_summary_warning() {
    let footnotes_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:footnote>
    <w:p><w:r><w:t>Note para 1</w:t></w:r></w:p>
    <w:p><w:r><w:t>Note para 2</w:t></w:r></w:p>
  </w:footnote>
</w:footnotes>"#;

    let docx = build_docx_with_parts_and_rels(
        r#"<w:p><w:r><w:t>Body text</w:t></w:r></w:p>"#,
        None,
        None,
        &[
            r#"<Relationship Id="rIdFootnotes" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/>"#,
        ],
        &[("word/footnotes.xml", footnotes_xml)],
    );
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert!(result.warnings.iter().any(|warning| {
        warning.kind == WarningKind::PartialExtraction
            && warning
                .message
                .contains("footnote story unknown contains 2 note paragraphs emitted without a footnote/endnote id")
    }));
}

#[test]
fn decode_repeated_note_media_without_id_emit_summary_warning() {
    let footnotes_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
             xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
             xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
             xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
  <w:footnote>
    <w:p>
      <w:r>
        <w:drawing>
          <wp:inline>
            <wp:docPr id="1" name="Img1"/>
            <a:graphic>
              <a:graphicData>
                <pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
                  <pic:blipFill><a:blip r:embed="rIdImg"/></pic:blipFill>
                </pic:pic>
              </a:graphicData>
            </a:graphic>
          </wp:inline>
        </w:drawing>
      </w:r>
    </w:p>
    <w:p>
      <w:r>
        <w:drawing>
          <wp:inline>
            <wp:docPr id="2" name="Img2"/>
            <a:graphic>
              <a:graphicData>
                <pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
                  <pic:blipFill><a:blip r:embed="rIdImg"/></pic:blipFill>
                </pic:pic>
              </a:graphicData>
            </a:graphic>
          </wp:inline>
        </w:drawing>
      </w:r>
    </w:p>
  </w:footnote>
</w:footnotes>"#;
    let footnotes_rels = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdImg" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/note.png"/>
</Relationships>"#;
    let png_bytes: &[u8] = &[
        0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x00,
    ];

    let docx = build_docx_with_parts_and_rels(
        r#"<w:p><w:r><w:t>Body text</w:t></w:r></w:p>"#,
        None,
        None,
        &[
            r#"<Relationship Id="rIdFootnotes" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/>"#,
        ],
        &[
            ("word/footnotes.xml", footnotes_xml),
            ("word/_rels/footnotes.xml.rels", footnotes_rels),
            ("word/media/note.png", png_bytes),
        ],
    );
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert!(result.warnings.iter().any(|warning| {
        warning.kind == WarningKind::PartialExtraction
            && warning
                .message
                .contains("footnote story unknown contains 2 note media items emitted without a footnote/endnote id")
    }));
}

#[test]
fn decode_malformed_reference_warning_is_story_scoped() {
    let comments_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:comment w:id="9">
    <w:p>
      <w:commentReference/>
    </w:p>
  </w:comment>
</w:comments>"#;

    let docx = build_docx_with_parts_and_rels(
        r#"<w:p><w:r><w:t>Body text</w:t></w:r></w:p>"#,
        None,
        None,
        &[
            r#"<Relationship Id="rIdComments" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/>"#,
        ],
        &[("word/comments.xml", comments_xml)],
    );
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert!(result.warnings.iter().any(|warning| {
        warning.kind == WarningKind::MalformedContent
            && warning.message.contains("comment story 9")
            && warning.message.contains("missing w:id")
    }));
}

#[test]
fn decode_repeated_malformed_references_in_comment_story_emit_summary_warning() {
    let comments_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:comment w:id="9">
    <w:p><w:commentReference/></w:p>
    <w:p><w:commentReference/></w:p>
  </w:comment>
</w:comments>"#;

    let docx = build_docx_with_parts_and_rels(
        r#"<w:p><w:r><w:t>Body text</w:t></w:r></w:p>"#,
        None,
        None,
        &[
            r#"<Relationship Id="rIdComments" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/>"#,
        ],
        &[("word/comments.xml", comments_xml)],
    );
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert!(result.warnings.iter().any(|warning| {
        warning.kind == WarningKind::MalformedContent
            && warning.message.contains(
                "comment story 9 contains 2 malformed note/comment references missing w:id",
            )
    }));
}

#[test]
fn decode_repeated_unresolved_relationships_in_comment_story_emit_summary_warning() {
    let comments_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:comment w:id="9">
    <w:p>
      <w:hyperlink r:id="rId404">
        <w:r><w:t>Broken 1</w:t></w:r>
      </w:hyperlink>
    </w:p>
    <w:p>
      <w:hyperlink r:id="rId405">
        <w:r><w:t>Broken 2</w:t></w:r>
      </w:hyperlink>
    </w:p>
  </w:comment>
</w:comments>"#;

    let docx = build_docx_with_parts_and_rels(
        r#"<w:p><w:r><w:t>Body text</w:t></w:r></w:p>"#,
        None,
        None,
        &[
            r#"<Relationship Id="rIdComments" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/>"#,
        ],
        &[("word/comments.xml", comments_xml)],
    );
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert!(result.warnings.iter().any(|warning| {
        warning.kind == WarningKind::UnresolvedRelationship
            && warning
                .message
                .contains("comment story 9 contains 2 unresolved relationships")
    }));
}
