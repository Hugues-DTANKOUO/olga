use super::*;

#[test]
fn decode_missing_media_warning_is_story_and_cell_scoped() {
    let footnotes_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
             xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
             xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
             xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
  <w:footnote w:id="2">
    <w:tbl>
      <w:tr>
        <w:tc>
          <w:p>
            <w:r>
              <w:drawing>
                <wp:inline>
                  <wp:docPr id="1" name="MissingImage"/>
                  <a:graphic>
                    <a:graphicData>
                      <pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
                        <pic:blipFill>
                          <a:blip r:embed="rIdMissing"/>
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
        warning.kind == WarningKind::MissingMedia
            && warning
                .message
                .contains("footnote story 2 table cell (0, 0)")
            && warning.message.contains("rIdMissing")
    }));
}

#[test]
fn decode_repeated_missing_media_in_header_table_cell_emits_summary_warning() {
    let header_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
       xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
       xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
  <w:tbl>
    <w:tr>
      <w:tc>
        <w:p>
          <w:r>
            <w:drawing>
              <wp:inline>
                <wp:docPr id="1" name="MissingImage1"/>
                <a:graphic>
                  <a:graphicData>
                    <pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
                      <pic:blipFill><a:blip r:embed="rIdMissing1"/></pic:blipFill>
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
                <wp:docPr id="2" name="MissingImage2"/>
                <a:graphic>
                  <a:graphicData>
                    <pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
                      <pic:blipFill><a:blip r:embed="rIdMissing2"/></pic:blipFill>
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
        warning.kind == WarningKind::MissingMedia
            && warning
                .message
                .contains("header story table cell (0, 0) contains 2 missing media references")
    }));
}

#[test]
fn decode_tracked_change_warning_is_story_scoped() {
    let comments_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:comment w:id="9">
    <w:p>
      <w:del w:id="1" w:author="Test">
        <w:r><w:t>Removed</w:t></w:r>
      </w:del>
      <w:r><w:t>Kept</w:t></w:r>
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
        warning.kind == WarningKind::TrackedChangeResolved
            && warning.message.contains("comment story 9")
            && warning.message.contains("tracked change")
    }));
}

#[test]
fn decode_repeated_tracked_deletions_in_comment_story_emit_summary_warning() {
    let comments_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:comment w:id="9">
    <w:p>
      <w:del w:id="1" w:author="Test"><w:r><w:t>Removed 1</w:t></w:r></w:del>
    </w:p>
    <w:p>
      <w:del w:id="2" w:author="Test"><w:r><w:t>Removed 2</w:t></w:r></w:del>
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
        warning.kind == WarningKind::TrackedChangeResolved
            && warning
                .message
                .contains("comment story 9 contains 2 skipped deletions from tracked changes")
    }));
}

#[test]
fn decode_repeated_tracked_insertions_in_header_cell_emit_summary_warning() {
    let header_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:tbl>
    <w:tr>
      <w:tc>
        <w:p>
          <w:ins w:id="1" w:author="Test"><w:r><w:t>Inserted 1</w:t></w:r></w:ins>
        </w:p>
        <w:p>
          <w:ins w:id="2" w:author="Test"><w:r><w:t>Inserted 2</w:t></w:r></w:ins>
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
        warning.kind == WarningKind::TrackedChangeResolved
            && warning
                .message
                .contains("header story table cell (0, 0) contains 2 accepted insertions from tracked changes")
    }));
}

#[test]
fn decode_repeated_ole_placeholders_in_header_cell_emit_summary_warning() {
    let header_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:tbl>
    <w:tr>
      <w:tc>
        <w:p><w:r><w:OLEObject/></w:r></w:p>
        <w:p><w:r><w:OLEObject/></w:r></w:p>
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
        warning.kind == WarningKind::UnsupportedElement
            && warning
                .message
                .contains("header story table cell (0, 0) contains 2 embedded OLE objects emitted as placeholder primitives")
    }));
}
