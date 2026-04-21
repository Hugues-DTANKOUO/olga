use std::io::{Cursor, Write};

use olga::formats::xlsx::XlsxDecoder;
use olga::traits::{DecodeResult, FormatDecoder};

pub fn decode_xlsx(data: &[u8]) -> DecodeResult {
    XlsxDecoder::new().decode(data.to_vec()).unwrap()
}

/// Build a minimal valid XLSX file in memory from sheet data.
/// Each sheet is a Vec of rows, each row is a Vec of cell strings.
pub fn build_xlsx(sheets: &[(&str, Vec<Vec<&str>>)]) -> Vec<u8> {
    let buf = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buf);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    let mut content_types = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>"#,
    );
    for (idx, _) in sheets.iter().enumerate() {
        content_types.push_str(&format!(
            r#"
  <Override PartName="/xl/worksheets/sheet{}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#,
            idx + 1
        ));
    }
    content_types.push_str("\n  <Override PartName=\"/xl/sharedStrings.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml\"/>");
    content_types.push_str("\n</Types>");
    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(content_types.as_bytes()).unwrap();

    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#,
    )
    .unwrap();

    let mut wb = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>"#,
    );
    for (idx, (name, _)) in sheets.iter().enumerate() {
        wb.push_str(&format!(
            "\n    <sheet name=\"{}\" sheetId=\"{}\" r:id=\"rId{}\"/>",
            name,
            idx + 1,
            idx + 1
        ));
    }
    wb.push_str("\n  </sheets>\n</workbook>");
    zip.start_file("xl/workbook.xml", options).unwrap();
    zip.write_all(wb.as_bytes()).unwrap();

    let mut wb_rels = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    );
    for (idx, _) in sheets.iter().enumerate() {
        wb_rels.push_str(&format!(
            "\n  <Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet{}.xml\"/>",
            idx + 1,
            idx + 1
        ));
    }
    wb_rels.push_str(&format!(
        "\n  <Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings\" Target=\"sharedStrings.xml\"/>",
        sheets.len() + 1
    ));
    wb_rels.push_str("\n</Relationships>");
    zip.start_file("xl/_rels/workbook.xml.rels", options)
        .unwrap();
    zip.write_all(wb_rels.as_bytes()).unwrap();

    let mut all_strings: Vec<String> = Vec::new();
    for rows in sheets.iter().map(|(_, rows)| rows) {
        for row in rows {
            for cell in row {
                let s = cell.to_string();
                if !all_strings.contains(&s) {
                    all_strings.push(s);
                }
            }
        }
    }

    let mut ss = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<sst xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" count=\"{}\" uniqueCount=\"{}\">",
        all_strings.len(),
        all_strings.len()
    );
    for s in &all_strings {
        ss.push_str(&format!("\n  <si><t>{}</t></si>", s));
    }
    ss.push_str("\n</sst>");
    zip.start_file("xl/sharedStrings.xml", options).unwrap();
    zip.write_all(ss.as_bytes()).unwrap();

    for (idx, (_, rows)) in sheets.iter().enumerate() {
        let mut sheet_xml = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">\n  <sheetData>",
        );
        for (r, row) in rows.iter().enumerate() {
            sheet_xml.push_str(&format!("\n    <row r=\"{}\">", r + 1));
            for (c, cell) in row.iter().enumerate() {
                let col_letter = col_to_letter(c as u32);
                let str_idx = all_strings.iter().position(|s| s == *cell).unwrap();
                sheet_xml.push_str(&format!(
                    "\n      <c r=\"{}{}\" t=\"s\"><v>{}</v></c>",
                    col_letter,
                    r + 1,
                    str_idx
                ));
            }
            sheet_xml.push_str("\n    </row>");
        }
        sheet_xml.push_str("\n  </sheetData>\n</worksheet>");
        let path = format!("xl/worksheets/sheet{}.xml", idx + 1);
        zip.start_file(&path, options).unwrap();
        zip.write_all(sheet_xml.as_bytes()).unwrap();
    }

    zip.finish().unwrap().into_inner()
}

pub fn build_xlsx_with_invalid_shared_strings() -> Vec<u8> {
    let buf = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buf);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/>
</Types>"#,
    )
    .unwrap();

    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#,
    )
    .unwrap();

    zip.start_file("xl/workbook.xml", options).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="BrokenStrings" sheetId="1" r:id="rId1"/>
  </sheets>
</workbook>"#,
    )
    .unwrap();

    zip.start_file("xl/_rels/workbook.xml.rels", options)
        .unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/>
</Relationships>"#,
    )
    .unwrap();

    zip.start_file("xl/worksheets/sheet1.xml", options).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1">
      <c r="A1" t="s"><v>0</v></c>
    </row>
  </sheetData>
</worksheet>"#,
    )
    .unwrap();

    zip.start_file("xl/sharedStrings.xml", options).unwrap();
    zip.write_all(br#"<sst><si><t>broken"#).unwrap();

    zip.finish().unwrap().into_inner()
}

pub fn col_to_letter(col: u32) -> String {
    let mut result = String::new();
    let mut c = col;
    loop {
        result.insert(0, (b'A' + (c % 26) as u8) as char);
        if c < 26 {
            break;
        }
        c = c / 26 - 1;
    }
    result
}
