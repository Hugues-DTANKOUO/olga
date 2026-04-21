use super::*;

#[test]
fn decode_multiple_blocks_in_table_cell_keep_cell_hint_and_inner_structure() {
    let numbering = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="0">
    <w:lvl w:ilvl="0"><w:numFmt w:val="bullet"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>
</w:numbering>"#;

    let body = r#"
    <w:tbl>
      <w:tr>
        <w:tc>
          <w:p><w:r><w:t>Intro paragraph</w:t></w:r></w:p>
          <w:p>
            <w:pPr><w:numPr><w:numId w:val="1"/><w:ilvl w:val="0"/></w:numPr></w:pPr>
            <w:r><w:t>Bullet inside cell</w:t></w:r>
          </w:p>
        </w:tc>
      </w:tr>
    </w:tbl>
    "#;

    let docx = build_docx_with_parts(body, None, Some(numbering), &[]);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    let intro = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Intro paragraph"))
        .unwrap();
    let bullet = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Bullet inside cell"))
        .unwrap();

    assert!(intro.hints.iter().any(|hint| {
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
        intro
            .hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::Paragraph))
    );
    assert!(bullet.hints.iter().any(|hint| {
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
    assert!(bullet.hints.iter().any(|hint| {
        matches!(
            hint.kind,
            HintKind::ListItem {
                depth: 0,
                ordered: false,
                ..
            }
        )
    }));
}

#[test]
fn decode_table_cell_paragraph_preserves_multiple_links_and_note_hints() {
    let body = r#"
    <w:tbl>
      <w:tr>
        <w:tc>
          <w:p>
            <w:hyperlink r:id="rIdLink1">
              <w:r><w:t>First</w:t></w:r>
            </w:hyperlink>
            <w:r><w:t> and </w:t></w:r>
            <w:hyperlink r:id="rIdLink2">
              <w:r><w:t>Second</w:t></w:r>
            </w:hyperlink>
            <w:r><w:footnoteReference w:id="5"/></w:r>
            <w:r><w:commentReference w:id="7"/></w:r>
          </w:p>
        </w:tc>
      </w:tr>
    </w:tbl>
    "#;

    let rels = [
        r#"<Relationship Id="rIdLink1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/one" TargetMode="External"/>"#,
        r#"<Relationship Id="rIdLink2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/two" TargetMode="External"/>"#,
    ];

    let docx = build_docx_with_parts_and_rels(body, None, None, &rels, &[]);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    let paragraph = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("First and Second"))
        .unwrap();

    assert!(paragraph.hints.iter().any(|hint| {
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
        paragraph
            .hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::Paragraph))
    );
    assert!(paragraph.hints.iter().any(|hint| {
        matches!(
            hint.kind,
            HintKind::Link { ref url } if url == "https://example.com/one"
        )
    }));
    assert!(paragraph.hints.iter().any(|hint| {
        matches!(
            hint.kind,
            HintKind::Link { ref url } if url == "https://example.com/two"
        )
    }));
    assert!(
        paragraph
            .hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::Footnote { ref id } if id == "5" ))
    );
    assert!(
        paragraph
            .hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::Sidebar))
    );
}
