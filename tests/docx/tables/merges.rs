use super::*;

#[test]
fn decode_table_vertical_merge_projects_rowspan_on_merged_cells() {
    let body = r#"
    <w:tbl>
      <w:tr>
        <w:tc>
          <w:tcPr><w:vMerge w:val="restart"/></w:tcPr>
          <w:p><w:r><w:t>Top</w:t></w:r></w:p>
        </w:tc>
      </w:tr>
      <w:tr>
        <w:tc>
          <w:tcPr><w:vMerge/></w:tcPr>
          <w:p><w:r><w:t>Bottom</w:t></w:r></w:p>
        </w:tc>
      </w:tr>
    </w:tbl>
    "#;

    let docx = build_docx(body);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    let top = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Top"))
        .unwrap();
    let bottom = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Bottom"))
        .unwrap();

    assert!(top.hints.iter().any(|hint| {
        matches!(
            hint.kind,
            HintKind::TableCell {
                row: 0,
                col: 0,
                rowspan: 2,
                colspan: 1,
            }
        )
    }));
    assert!(bottom.hints.iter().any(|hint| {
        matches!(
            hint.kind,
            HintKind::TableCell {
                row: 0,
                col: 0,
                rowspan: 2,
                colspan: 1,
            }
        )
    }));
    assert!(
        !result
            .warnings
            .iter()
            .any(|warning| warning.message.contains("vertical table merges"))
    );
}

#[test]
fn decode_table_vertical_merge_without_restart_warns() {
    let body = r#"
    <w:tbl>
      <w:tr>
        <w:tc>
          <w:tcPr><w:vMerge/></w:tcPr>
          <w:p><w:r><w:t>Dangling</w:t></w:r></w:p>
        </w:tc>
      </w:tr>
    </w:tbl>
    "#;

    let docx = build_docx(body);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert!(result.warnings.iter().any(|warning| {
        warning.kind == WarningKind::PartialExtraction
            && warning
                .message
                .contains("continues a vertical merge without a matching restart")
    }));
}
