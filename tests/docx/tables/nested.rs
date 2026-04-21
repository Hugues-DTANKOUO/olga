use super::*;

#[test]
fn decode_nested_tables_warn_and_keep_inner_table_hints() {
    let body = r#"
    <w:tbl>
      <w:tr>
        <w:tc>
          <w:p><w:r><w:t>Outer intro</w:t></w:r></w:p>
          <w:tbl>
            <w:tr>
              <w:tc><w:p><w:r><w:t>Inner A</w:t></w:r></w:p></w:tc>
              <w:tc><w:p><w:r><w:t>Inner B</w:t></w:r></w:p></w:tc>
            </w:tr>
            <w:tr>
              <w:tc><w:p><w:r><w:t>Inner C</w:t></w:r></w:p></w:tc>
            </w:tr>
          </w:tbl>
        </w:tc>
        <w:tc><w:p><w:r><w:t>Outer sibling</w:t></w:r></w:p></w:tc>
      </w:tr>
    </w:tbl>
    "#;

    let docx = build_docx(body);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    let outer_intro = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Outer intro"))
        .unwrap();
    let inner_c = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Inner C"))
        .unwrap();
    let outer_sibling = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Outer sibling"))
        .unwrap();

    assert!(outer_intro.hints.iter().any(|hint| {
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
    assert!(
        outer_intro
            .hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::Paragraph))
    );
    assert!(inner_c.hints.iter().any(|hint| {
        matches!(
            hint.kind,
            HintKind::TableCell {
                row: 1,
                col: 0,
                rowspan: 1,
                colspan: 1,
            }
        )
    }));
    assert!(inner_c.hints.iter().any(|hint| {
        matches!(
            hint.kind,
            HintKind::ContainedInTableCell {
                row: 1,
                col: 0,
                depth: 0,
            }
        )
    }));
    assert!(inner_c.hints.iter().any(|hint| {
        matches!(
            hint.kind,
            HintKind::FlattenedFromTableCell {
                row: 0,
                col: 0,
                depth: 1,
            }
        )
    }));
    assert!(outer_sibling.hints.iter().any(|hint| {
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
    assert!(result.warnings.iter().any(|warning| {
        warning.kind == WarningKind::PartialExtraction
            && warning
                .message
                .contains("table cell (0, 0) contains a nested table")
    }));
}

#[test]
fn decode_nested_table_as_first_cell_content_emits_flattened_container_hint() {
    let body = r#"
    <w:tbl>
      <w:tr>
        <w:tc>
          <w:tbl>
            <w:tr>
              <w:tc><w:p><w:r><w:t>Inner first</w:t></w:r></w:p></w:tc>
            </w:tr>
          </w:tbl>
        </w:tc>
      </w:tr>
    </w:tbl>
    "#;

    let docx = build_docx(body);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    let inner = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Inner first"))
        .unwrap();

    assert!(inner.hints.iter().any(|hint| {
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
    assert!(inner.hints.iter().any(|hint| {
        matches!(
            hint.kind,
            HintKind::ContainedInTableCell {
                row: 0,
                col: 0,
                depth: 1,
            }
        )
    }));
    assert!(inner.hints.iter().any(|hint| {
        matches!(
            hint.kind,
            HintKind::FlattenedFromTableCell {
                row: 0,
                col: 0,
                depth: 1,
            }
        )
    }));
    assert!(result.warnings.iter().any(|warning| {
        warning.kind == WarningKind::PartialExtraction
            && warning
                .message
                .contains("table cell (0, 0) contains a nested table")
    }));
}

#[test]
fn decode_deeply_nested_tables_emit_flattening_chain() {
    let body = r#"
    <w:tbl>
      <w:tr>
        <w:tc>
          <w:tbl>
            <w:tr>
              <w:tc><w:p><w:r><w:t>Inner anchor</w:t></w:r></w:p></w:tc>
              <w:tc>
                <w:tbl>
                  <w:tr>
                    <w:tc><w:p><w:r><w:t>Deep cell</w:t></w:r></w:p></w:tc>
                  </w:tr>
                </w:tbl>
              </w:tc>
            </w:tr>
          </w:tbl>
        </w:tc>
      </w:tr>
    </w:tbl>
    "#;

    let docx = build_docx(body);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    let deep = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Deep cell"))
        .unwrap();

    assert!(deep.hints.iter().any(|hint| matches!(
        hint.kind,
        HintKind::TableCell {
            row: 0,
            col: 0,
            rowspan: 1,
            colspan: 1,
        }
    )));
    assert!(deep.hints.iter().any(|hint| matches!(
        hint.kind,
        HintKind::ContainedInTableCell {
            row: 0,
            col: 0,
            depth: 0,
        }
    )));
    assert!(deep.hints.iter().any(|hint| matches!(
        hint.kind,
        HintKind::FlattenedFromTableCell {
            row: 0,
            col: 0,
            depth: 1,
        }
    )));
    assert!(deep.hints.iter().any(|hint| matches!(
        hint.kind,
        HintKind::FlattenedFromTableCell {
            row: 0,
            col: 1,
            depth: 2,
        }
    )));
    assert!(result.warnings.iter().any(|warning| {
        warning.kind == WarningKind::PartialExtraction
            && warning
                .message
                .contains("table cell (0, 0) required flattening of 2 nested tables up to depth 2")
    }));
}

#[test]
fn decode_multiple_nested_tables_in_same_cell_emit_summary_warning() {
    let body = r#"
    <w:tbl>
      <w:tr>
        <w:tc>
          <w:tbl>
            <w:tr>
              <w:tc><w:p><w:r><w:t>Inner one</w:t></w:r></w:p></w:tc>
            </w:tr>
          </w:tbl>
          <w:tbl>
            <w:tr>
              <w:tc><w:p><w:r><w:t>Inner two</w:t></w:r></w:p></w:tc>
            </w:tr>
          </w:tbl>
        </w:tc>
      </w:tr>
    </w:tbl>
    "#;

    let docx = build_docx(body);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert!(
        result
            .primitives
            .iter()
            .any(|primitive| primitive.text_content() == Some("Inner one"))
    );
    assert!(
        result
            .primitives
            .iter()
            .any(|primitive| primitive.text_content() == Some("Inner two"))
    );
    assert!(result.warnings.iter().any(|warning| {
        warning.kind == WarningKind::PartialExtraction
            && warning
                .message
                .contains("table cell (0, 0) required flattening of 2 nested tables up to depth 1")
    }));
}

#[test]
fn decode_nested_tables_in_header_cell_emit_story_scoped_flattening_warning() {
    let header_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:tbl>
    <w:tr>
      <w:tc>
        <w:tbl>
          <w:tr>
            <w:tc><w:p><w:r><w:t>Header inner one</w:t></w:r></w:p></w:tc>
          </w:tr>
        </w:tbl>
        <w:tbl>
          <w:tr>
            <w:tc><w:p><w:r><w:t>Header inner two</w:t></w:r></w:p></w:tc>
          </w:tr>
        </w:tbl>
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

    assert!(
        result
            .primitives
            .iter()
            .any(|primitive| primitive.text_content() == Some("Header inner one"))
    );
    assert!(
        result
            .primitives
            .iter()
            .any(|primitive| primitive.text_content() == Some("Header inner two"))
    );
    assert!(result.warnings.iter().any(|warning| {
        warning.kind == WarningKind::PartialExtraction
            && warning
                .message
                .contains("header story table cell (0, 0) required flattening of 2 nested tables up to depth 1")
    }));
}

#[test]
fn decode_nested_tables_in_footnote_emit_story_id_scoped_flattening_warning() {
    let footnotes_xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:footnote w:id="2">
    <w:tbl>
      <w:tr>
        <w:tc>
          <w:tbl>
            <w:tr>
              <w:tc><w:p><w:r><w:t>Fn inner one</w:t></w:r></w:p></w:tc>
            </w:tr>
          </w:tbl>
          <w:tbl>
            <w:tr>
              <w:tc><w:p><w:r><w:t>Fn inner two</w:t></w:r></w:p></w:tc>
            </w:tr>
          </w:tbl>
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

    assert!(
        result
            .primitives
            .iter()
            .any(|primitive| primitive.text_content() == Some("Fn inner one"))
    );
    assert!(
        result
            .primitives
            .iter()
            .any(|primitive| primitive.text_content() == Some("Fn inner two"))
    );
    assert!(result.warnings.iter().any(|warning| {
        warning.kind == WarningKind::PartialExtraction
            && warning
                .message
                .contains("footnote story 2 table cell (0, 0) required flattening of 2 nested tables up to depth 1")
    }));
}
