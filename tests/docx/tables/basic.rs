use super::*;

#[test]
fn decode_table() {
    let body = r#"
    <w:tbl>
      <w:tr>
        <w:tc><w:p><w:r><w:t>A1</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>B1</w:t></w:r></w:p></w:tc>
      </w:tr>
      <w:tr>
        <w:tc><w:p><w:r><w:t>A2</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>B2</w:t></w:r></w:p></w:tc>
      </w:tr>
    </w:tbl>
    "#;

    let docx = build_docx(body);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    assert_eq!(result.primitives.len(), 4);

    let check_cell = |idx: usize, expected_row: u32, expected_col: u32, expected_text: &str| {
        let prim = &result.primitives[idx];
        assert_eq!(prim.text_content(), Some(expected_text));
        assert!(prim.hints.iter().any(|h| matches!(
            h.kind,
            HintKind::TableCell { row, col, .. } if row == expected_row && col == expected_col
        )));
    };

    check_cell(0, 0, 0, "A1");
    check_cell(1, 0, 1, "B1");
    check_cell(2, 1, 0, "A2");
    check_cell(3, 1, 1, "B2");
}

#[test]
fn decode_table_header_rows_from_native_tbl_header() {
    let body = r#"
    <w:tbl>
      <w:tr>
        <w:trPr><w:tblHeader/></w:trPr>
        <w:tc><w:p><w:r><w:t>Name</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>Role</w:t></w:r></w:p></w:tc>
      </w:tr>
      <w:tr>
        <w:tc><w:p><w:r><w:t>Alice</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>Engineer</w:t></w:r></w:p></w:tc>
      </w:tr>
    </w:tbl>
    "#;

    let docx = build_docx(body);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    let name = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Name"))
        .unwrap();
    let role = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Role"))
        .unwrap();
    let alice = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Alice"))
        .unwrap();

    assert!(
        name.hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::TableHeader { col: 0 }))
    );
    assert!(
        role.hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::TableHeader { col: 1 }))
    );
    assert!(alice.hints.iter().any(|hint| {
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
    assert!(
        !result
            .warnings
            .iter()
            .any(|warning| warning.kind == WarningKind::HeuristicInference)
    );
}

#[test]
fn decode_table_grid_span_updates_colspan_and_column_positions() {
    let body = r#"
    <w:tbl>
      <w:tr>
        <w:tc>
          <w:tcPr><w:gridSpan w:val="2"/></w:tcPr>
          <w:p><w:r><w:t>Merged</w:t></w:r></w:p>
        </w:tc>
        <w:tc><w:p><w:r><w:t>Right</w:t></w:r></w:p></w:tc>
      </w:tr>
      <w:tr>
        <w:tc><w:p><w:r><w:t>A2</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>B2</w:t></w:r></w:p></w:tc>
        <w:tc><w:p><w:r><w:t>C2</w:t></w:r></w:p></w:tc>
      </w:tr>
    </w:tbl>
    "#;

    let docx = build_docx(body);
    let decoder = DocxDecoder::new();
    let result = decoder.decode(docx).unwrap();

    let merged = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Merged"))
        .unwrap();
    let right = result
        .primitives
        .iter()
        .find(|primitive| primitive.text_content() == Some("Right"))
        .unwrap();

    assert!(merged.hints.iter().any(|hint| {
        matches!(
            hint.kind,
            HintKind::TableCell {
                row: 0,
                col: 0,
                rowspan: 1,
                colspan: 2,
            }
        )
    }));
    assert!(right.hints.iter().any(|hint| {
        matches!(
            hint.kind,
            HintKind::TableCell {
                row: 0,
                col: 2,
                rowspan: 1,
                colspan: 1,
            }
        )
    }));
}
