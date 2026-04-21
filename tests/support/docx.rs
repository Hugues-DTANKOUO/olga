#![allow(dead_code)]

use std::io::Write;

pub fn build_docx(body_xml: &str) -> Vec<u8> {
    build_docx_with_parts(body_xml, None, None, &[])
}

pub fn build_docx_with_parts(
    body_xml: &str,
    styles_xml: Option<&str>,
    numbering_xml: Option<&str>,
    extra_parts: &[(&str, &[u8])],
) -> Vec<u8> {
    build_docx_with_parts_and_rels(body_xml, styles_xml, numbering_xml, &[], extra_parts)
}

pub fn build_docx_with_parts_and_rels(
    body_xml: &str,
    styles_xml: Option<&str>,
    numbering_xml: Option<&str>,
    extra_doc_rels: &[&str],
    extra_parts: &[(&str, &[u8])],
) -> Vec<u8> {
    let buf = Vec::new();
    let cursor = std::io::Cursor::new(buf);
    let mut zip = zip::ZipWriter::new(cursor);

    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    let content_types = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Default Extension="jpeg" ContentType="image/jpeg"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#;
    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(content_types.as_bytes()).unwrap();

    let root_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#;
    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(root_rels.as_bytes()).unwrap();

    let document_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
            xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
            xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
            xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"
            xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"
            xmlns:v="urn:schemas-microsoft-com:vml">
  <w:body>
    {}
  </w:body>
</w:document>"#,
        body_xml
    );
    zip.start_file("word/document.xml", options).unwrap();
    zip.write_all(document_xml.as_bytes()).unwrap();

    let mut rels_entries = Vec::new();
    if styles_xml.is_some() {
        rels_entries.push(r#"<Relationship Id="rIdStyles" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>"#);
    }
    if numbering_xml.is_some() {
        rels_entries.push(r#"<Relationship Id="rIdNum" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/>"#);
    }
    rels_entries.extend(extra_doc_rels.iter().copied());
    let doc_rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  {}
</Relationships>"#,
        rels_entries.join("\n  ")
    );
    zip.start_file("word/_rels/document.xml.rels", options)
        .unwrap();
    zip.write_all(doc_rels.as_bytes()).unwrap();

    if let Some(styles) = styles_xml {
        zip.start_file("word/styles.xml", options).unwrap();
        zip.write_all(styles.as_bytes()).unwrap();
    }

    if let Some(numbering) = numbering_xml {
        zip.start_file("word/numbering.xml", options).unwrap();
        zip.write_all(numbering.as_bytes()).unwrap();
    }

    for (path, data) in extra_parts {
        zip.start_file(*path, options).unwrap();
        zip.write_all(data).unwrap();
    }

    let cursor = zip.finish().unwrap();
    cursor.into_inner()
}
