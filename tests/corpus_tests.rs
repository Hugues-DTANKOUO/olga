use std::io::{Cursor, Write};

use olga::formats::docx::DocxDecoder;
use olga::formats::html::HtmlDecoder;
use olga::formats::pdf::PdfDecoder;
use olga::formats::xlsx::XlsxDecoder;
use olga::model::{GeometrySpace, PaginationBasis, ValueOrigin};

mod support;

use support::contracts::{assert_contract, assert_decode_stable};

fn build_docx(body_xml: &str) -> Vec<u8> {
    let styles_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="Heading1">
    <w:name w:val="Heading 1"/>
    <w:pPr><w:outlineLvl w:val="0"/></w:pPr>
    <w:rPr><w:b/><w:sz w:val="32"/></w:rPr>
  </w:style>
</w:styles>"#;

    let buf = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buf);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#,
    )
    .unwrap();

    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#,
    )
    .unwrap();

    let document_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>{}</w:body>
</w:document>"#,
        body_xml
    );
    zip.start_file("word/document.xml", options).unwrap();
    zip.write_all(document_xml.as_bytes()).unwrap();

    zip.start_file("word/_rels/document.xml.rels", options)
        .unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdStyles" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>"#,
    )
    .unwrap();

    zip.start_file("word/styles.xml", options).unwrap();
    zip.write_all(styles_xml.as_bytes()).unwrap();

    zip.finish().unwrap().into_inner()
}

fn build_pdf(text: &str) -> Vec<u8> {
    let escaped = text
        .replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)");
    let content_stream = format!("BT /F1 12 Tf 72 720 Td ({}) Tj ET", escaped);
    let content_length = content_stream.len();

    let mut pdf = String::new();
    pdf.push_str("%PDF-1.4\n");

    let obj1_offset = pdf.len();
    pdf.push_str("1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

    let obj2_offset = pdf.len();
    pdf.push_str("2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");

    let obj3_offset = pdf.len();
    pdf.push_str(
        "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n",
    );

    let obj4_offset = pdf.len();
    pdf.push_str(&format!(
        "4 0 obj\n<< /Length {} >>\nstream\n{}\nendstream\nendobj\n",
        content_length, content_stream
    ));

    let obj5_offset = pdf.len();
    pdf.push_str("5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n");

    let xref_offset = pdf.len();
    pdf.push_str("xref\n0 6\n");
    pdf.push_str("0000000000 65535 f \n");
    pdf.push_str(&format!("{:010} 00000 n \n", obj1_offset));
    pdf.push_str(&format!("{:010} 00000 n \n", obj2_offset));
    pdf.push_str(&format!("{:010} 00000 n \n", obj3_offset));
    pdf.push_str(&format!("{:010} 00000 n \n", obj4_offset));
    pdf.push_str(&format!("{:010} 00000 n \n", obj5_offset));
    pdf.push_str(&format!(
        "trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
        xref_offset
    ));

    pdf.into_bytes()
}

fn build_xlsx(tsv: &str) -> Vec<u8> {
    let rows: Vec<Vec<&str>> = tsv.lines().map(|line| line.split('\t').collect()).collect();
    let sheets = [("Corpus", rows)];

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
    <sheet name="Corpus" sheetId="1" r:id="rId1"/>
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

    let mut all_strings: Vec<String> = Vec::new();
    for row in sheets.iter().flat_map(|(_, rows)| rows.iter()) {
        for cell in row {
            let text = (*cell).to_string();
            if !all_strings.contains(&text) {
                all_strings.push(text);
            }
        }
    }

    let mut shared_strings = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<sst xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" count=\"{}\" uniqueCount=\"{}\">",
        all_strings.len(),
        all_strings.len()
    );
    for value in &all_strings {
        shared_strings.push_str(&format!("\n  <si><t>{}</t></si>", value));
    }
    shared_strings.push_str("\n</sst>");
    zip.start_file("xl/sharedStrings.xml", options).unwrap();
    zip.write_all(shared_strings.as_bytes()).unwrap();

    let mut sheet_xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">\n  <sheetData>",
    );
    for (row_idx, row) in sheets[0].1.iter().enumerate() {
        sheet_xml.push_str(&format!("\n    <row r=\"{}\">", row_idx + 1));
        for (col_idx, cell) in row.iter().enumerate() {
            let col_letter = ((b'A' + col_idx as u8) as char).to_string();
            let str_idx = all_strings.iter().position(|value| value == cell).unwrap();
            sheet_xml.push_str(&format!(
                "\n      <c r=\"{}{}\" t=\"s\"><v>{}</v></c>",
                col_letter,
                row_idx + 1,
                str_idx
            ));
        }
        sheet_xml.push_str("\n    </row>");
    }
    sheet_xml.push_str("\n  </sheetData>\n</worksheet>");
    zip.start_file("xl/worksheets/sheet1.xml", options).unwrap();
    zip.write_all(sheet_xml.as_bytes()).unwrap();

    zip.finish().unwrap().into_inner()
}

#[test]
fn corpus_html_fixture_is_stable() {
    let html = include_str!("corpus/html/semantic_email.html");
    let decoder = HtmlDecoder::new();
    let first = assert_decode_stable(&decoder, html.as_bytes());

    assert_contract(&first);
    assert_eq!(
        first.metadata.page_count_provenance.basis,
        PaginationBasis::SingleLogicalPage
    );
    assert_eq!(
        first.metadata.page_count_provenance.origin,
        ValueOrigin::Synthetic
    );
    assert!(first.warnings.is_empty());
    assert!(first.primitives.iter().all(|primitive| {
        primitive.geometry_provenance.origin == ValueOrigin::Synthetic
            && primitive.geometry_provenance.space == GeometrySpace::DomFlow
    }));
    assert!(
        first
            .primitives
            .iter()
            .any(|primitive| primitive.text_content() == Some("Customer Update"))
    );
}

#[test]
fn corpus_docx_fixture_is_stable() {
    let body = include_str!("corpus/docx/body.xml");
    let data = build_docx(body);
    let decoder = DocxDecoder::new();
    let first = assert_decode_stable(&decoder, &data);

    assert_contract(&first);
    assert_eq!(
        first.metadata.page_count_provenance.origin,
        ValueOrigin::Estimated
    );
    assert!(first.warnings.is_empty());
    assert!(first.primitives.iter().all(|primitive| {
        primitive.geometry_provenance.origin == ValueOrigin::Reconstructed
            && primitive.geometry_provenance.space == GeometrySpace::LogicalPage
    }));
    assert!(
        first
            .primitives
            .iter()
            .any(|primitive| primitive.text_content()
                == Some("Corpus paragraph for DOCX validation."))
    );
}

#[test]
fn corpus_pdf_fixture_is_stable() {
    let text = include_str!("corpus/pdf/text.txt").trim();
    let data = build_pdf(text);
    let decoder = PdfDecoder;
    let first = assert_decode_stable(&decoder, &data);

    assert_contract(&first);
    assert_eq!(
        first.metadata.page_count_provenance.basis,
        PaginationBasis::PhysicalPages
    );
    assert!(first.warnings.is_empty());
    assert!(first.primitives.iter().all(|primitive| {
        primitive.geometry_provenance.origin == ValueOrigin::Observed
            && primitive.geometry_provenance.space == GeometrySpace::PhysicalPage
    }));
}

#[test]
fn corpus_xlsx_fixture_is_stable() {
    let tsv = include_str!("corpus/xlsx/people.tsv");
    let data = build_xlsx(tsv);
    let decoder = XlsxDecoder::new();
    let first = assert_decode_stable(&decoder, &data);

    assert_contract(&first);
    assert_eq!(
        first.metadata.page_count_provenance.basis,
        PaginationBasis::WorkbookSheets
    );
    assert_eq!(first.warnings.len(), 1);
    assert_eq!(
        first.warnings[0].kind,
        olga::error::WarningKind::HeuristicInference
    );
    assert!(first.primitives.iter().all(|primitive| {
        primitive.geometry_provenance.origin == ValueOrigin::Reconstructed
            && primitive.geometry_provenance.space == GeometrySpace::LogicalGrid
    }));
    assert!(first.primitives.iter().any(|primitive| {
        primitive
            .text_content()
            .is_some_and(|text| text == "Alice" || text == "London")
    }));
}
