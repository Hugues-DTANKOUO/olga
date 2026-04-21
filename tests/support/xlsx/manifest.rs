//! Manifest fraction-format canary fixture for #48.
//!
//! Models the `Specs (imperial) '25` sheet of the production
//! `manifest.xlsx` document the user reported. The defect: cells in
//! column B (`Width (in)`) carry the format code `# ?/8"` so dimensions
//! like `96.125` should render as `96 1/8"`. The legacy pipeline
//! ignored the fraction grammar entirely and emitted the raw decimal,
//! which is exactly the regression this fixture pins.
//!
//! | Cell | Format code | Raw value | Expected |
//! |------|-------------|-----------|----------|
//! | B1   | `# ?/8"`    | 96.125    | `96 1/8"` |
//! | B2   | `# ?/8"`    | 96.000    | `96"`     |
//! | B3   | `# ?/8"`    | 96.250    | `96 1/4"` (reduced) |
//! | B4   | `# ?/8"`    | 96.125    | `96 1/8"` |
//! | B5   | `# ?/8"`    | 96.375    | `96 3/8"` |
//!
//! Row 3 pins the OpenXML reduction convention: Excel and LibreOffice
//! both collapse `2/8` → `1/4` via GCD even when the format code
//! specifies the literal denominator, so the renderer must follow suit.
//!
//! Like `watchlist.rs`, the fixture is intentionally minimal — one sheet,
//! one custom number format, five styled value cells, and the column-A
//! labels that anchor each row in the shared-string table.

use std::io::{Cursor, Write};

/// Build the canary `Specs (imperial) '25` workbook described in the
/// module docs.
pub fn build_manifest_fraction_canary_xlsx() -> Vec<u8> {
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
  <Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>
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
    <sheet name="Specs (imperial) '25" sheetId="1" r:id="rId1"/>
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
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>"#,
    )
    .unwrap();

    // Shared strings — one label per row anchoring the value cell.
    zip.start_file("xl/sharedStrings.xml", options).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="5" uniqueCount="5">
  <si><t>Panel-A</t></si>
  <si><t>Panel-B</t></si>
  <si><t>Panel-C</t></si>
  <si><t>Panel-D</t></si>
  <si><t>Panel-E</t></si>
</sst>"#,
    )
    .unwrap();

    // One custom number format (numFmtId 164) covering the fraction
    // canaries. `&quot;` decodes to the literal `"` inch-mark suffix.
    // cellXfs entries:
    //   0 = unstyled (numFmtId 0, the General default)
    //   1 = imperial fraction (numFmtId 164, `# ?/8"`)
    // Note: the bytestring uses `br##"…"##` delimiters because the
    // format code `# ?/8"` puts a `"` followed by `#` in the payload,
    // which would close a standard `br#"…"#` literal prematurely.
    zip.start_file("xl/styles.xml", options).unwrap();
    zip.write_all(
        br##"<?xml version="1.0" encoding="UTF-8"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <numFmts count="1">
    <numFmt numFmtId="164" formatCode="# ?/8&quot;"/>
  </numFmts>
  <cellXfs count="2">
    <xf numFmtId="0"/>
    <xf numFmtId="164"/>
  </cellXfs>
</styleSheet>"##,
    )
    .unwrap();

    // Five rows: column A = label (shared string), column B = the styled
    // imperial-fraction numeric cell. Values mirror the user's report.
    zip.start_file("xl/worksheets/sheet1.xml", options).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1">
      <c r="A1" t="s"><v>0</v></c>
      <c r="B1" s="1"><v>96.125</v></c>
    </row>
    <row r="2">
      <c r="A2" t="s"><v>1</v></c>
      <c r="B2" s="1"><v>96</v></c>
    </row>
    <row r="3">
      <c r="A3" t="s"><v>2</v></c>
      <c r="B3" s="1"><v>96.25</v></c>
    </row>
    <row r="4">
      <c r="A4" t="s"><v>3</v></c>
      <c r="B4" s="1"><v>96.125</v></c>
    </row>
    <row r="5">
      <c r="A5" t="s"><v>4</v></c>
      <c r="B5" s="1"><v>96.375</v></c>
    </row>
  </sheetData>
</worksheet>"#,
    )
    .unwrap();

    zip.finish().unwrap().into_inner()
}
