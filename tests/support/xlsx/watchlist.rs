//! Watchlist canary fixture for the POI-aligned pipeline end-to-end
//! validation (#47).
//!
//! Bundles the five real-world rendering defects observed on the
//! production `watchlist.xlsx` document into one synthetic workbook so
//! the assertions can be pinned in CI without shipping the original
//! file. Every canary corresponds to a specific failure on the legacy
//! pipeline that the POI-aligned renderer is meant to fix:
//!
//! | Cell | Format code            | Raw value         | Expected | Legacy bug                          |
//! |------|------------------------|-------------------|----------|--------------------------------------|
//! | B1   | `0.00E+00`             | `2.918e13`        | `2.92E+13` | rendered `29180000000000.00`       |
//! | B2   | `h:mm AM/PM`           | `0.39583333...`   | `9:30 AM`  | rendered `1899-12-31T09:30:00`     |
//! | B3   | `+0"bps";-0"bps"`      | `2`               | `+2bps`    | rendered `2bps` (no `+`)           |
//! | B4   | `+0"bps";-0"bps"`      | `-1`              | `-1bps`    | rendered `-+1bps` (double sign)    |
//! | B5   | `0.0"x"`               | `62.4`            | `62.4x`    | suffix dropped / wrong category    |
//!
//! The sixth canary — a comment on cell `Watchlist!F9` surfacing in the
//! emitted primitive stream — exercises the comments-on-empty-cells fix
//! from #46 in concert with the format pipeline (the comment lives on a
//! cell that has no `<c>` element of its own).

use std::io::{Cursor, Write};

/// Build the canary `Watchlist` workbook described in the module docs.
///
/// The fixture is intentionally minimal: one sheet, six anchored rows
/// (5 canary value cells + 1 comment-only cell), one shared string
/// per label, and one custom number format per canary in `<numFmts>`.
/// Anything not directly in the canary path (column widths, themes,
/// styles for fonts/borders) is omitted — the assertions key off
/// rendered text and surfaced comments only.
pub fn build_watchlist_canary_xlsx() -> Vec<u8> {
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
  <Override PartName="/xl/comments1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.comments+xml"/>
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
    <sheet name="Watchlist" sheetId="1" r:id="rId1"/>
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

    // Shared strings — one label per canary row plus a comment-only
    // anchor label so the row is referenced even when the value cell is
    // omitted.
    zip.start_file("xl/sharedStrings.xml", options).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="6" uniqueCount="6">
  <si><t>Scientific</t></si>
  <si><t>TimeOfDay</t></si>
  <si><t>BpsPositive</t></si>
  <si><t>BpsNegative</t></si>
  <si><t>SuffixX</t></si>
  <si><t>F9Note</t></si>
</sst>"#,
    )
    .unwrap();

    // Five custom number formats (numFmtId 164..168) cover the canaries.
    // `&quot;` decodes to `"` for the quoted-literal sections.
    // cellXfs entries:
    //   0 = unstyled (numFmtId 0, the General default)
    //   1 = scientific          (numFmtId 164, "0.00E+00")
    //   2 = h:mm AM/PM          (numFmtId 165, "h:mm AM/PM")
    //   3 = +0"bps";-0"bps"     (numFmtId 166, custom sign-bearing)
    //   4 = 0.0"x"              (numFmtId 167, literal suffix)
    zip.start_file("xl/styles.xml", options).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <numFmts count="4">
    <numFmt numFmtId="164" formatCode="0.00E+00"/>
    <numFmt numFmtId="165" formatCode="h:mm AM/PM"/>
    <numFmt numFmtId="166" formatCode="+0&quot;bps&quot;;-0&quot;bps&quot;"/>
    <numFmt numFmtId="167" formatCode="0.0&quot;x&quot;"/>
  </numFmts>
  <cellXfs count="5">
    <xf numFmtId="0"/>
    <xf numFmtId="164"/>
    <xf numFmtId="165"/>
    <xf numFmtId="166"/>
    <xf numFmtId="167"/>
  </cellXfs>
</styleSheet>"#,
    )
    .unwrap();

    // Canary sheet: column A = label (shared string), column B = the
    // styled numeric cell. Row 9 is annotation-only (no `<c>` element)
    // — its value lives entirely in the comment, exercising the empty-
    // cell-comment surface from #46.
    //
    // The time-of-day canary uses 0.39583333333333333, the Excel serial
    // for 9:30 AM (9.5 hours / 24).
    zip.start_file("xl/worksheets/sheet1.xml", options).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1">
      <c r="A1" t="s"><v>0</v></c>
      <c r="B1" s="1"><v>29180000000000</v></c>
    </row>
    <row r="2">
      <c r="A2" t="s"><v>1</v></c>
      <c r="B2" s="2"><v>0.39583333333333333</v></c>
    </row>
    <row r="3">
      <c r="A3" t="s"><v>2</v></c>
      <c r="B3" s="3"><v>2</v></c>
    </row>
    <row r="4">
      <c r="A4" t="s"><v>3</v></c>
      <c r="B4" s="3"><v>-1</v></c>
    </row>
    <row r="5">
      <c r="A5" t="s"><v>4</v></c>
      <c r="B5" s="4"><v>62.4</v></c>
    </row>
    <row r="9">
      <c r="A9" t="s"><v>5</v></c>
    </row>
  </sheetData>
</worksheet>"#,
    )
    .unwrap();

    zip.start_file("xl/worksheets/_rels/sheet1.xml.rels", options)
        .unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="../comments1.xml"/>
</Relationships>"#,
    )
    .unwrap();

    // Comment on F9 — the bare cell has no `<c>` element. Without the
    // union-iteration fix from #46 this annotation would never reach the
    // emit pass.
    zip.start_file("xl/comments1.xml", options).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<comments xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <authors>
    <author>Analyst</author>
  </authors>
  <commentList>
    <comment ref="F9" authorId="0"><text><t>watchlist F9 review note</t></text></comment>
  </commentList>
</comments>"#,
    )
    .unwrap();

    zip.finish().unwrap().into_inner()
}
