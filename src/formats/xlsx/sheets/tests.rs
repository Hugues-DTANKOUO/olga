use std::collections::BTreeMap;
use std::io::Write;

use super::raw::{comment_bounds, scan_raw_sheet_profile};
use super::*;
use crate::formats::xlsx::workbook_support::{
    WorkbookStyleProfile, load_shared_strings, parse_style_profile,
};

#[test]
fn determine_sheet_value_mode_prefers_raw_streaming_for_simple_cells() {
    let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1">
      <c r="A1" t="s"><v>0</v></c>
      <c r="B1" t="inlineStr"><is><t>beta</t></is></c>
      <c r="C1"><v>42</v></c>
      <c r="D1" t="b"><v>1</v></c>
    </row>
  </sheetData>
</worksheet>"#;

    let (mode, uses_shared_strings) =
        determine_sheet_value_mode(xml, &WorkbookStyleProfile::default());
    assert_eq!(mode, SheetValueMode::RawXmlStreaming);
    assert!(uses_shared_strings);
}

#[test]
fn determine_sheet_value_mode_keeps_raw_streaming_for_date_styled_numeric_cells() {
    // Under the per-cell rendering architecture the raw streaming path
    // formats dates, currency, percents, and other numeric formats in
    // place via the workbook style profile; we no longer need to escape
    // to the calamine workbook decoder for any styled numeric cell.
    let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1">
      <c r="A1" s="3"><v>45123</v></c>
    </row>
  </sheetData>
</worksheet>"#;

    let style_profile = WorkbookStyleProfile::default();
    let (mode, uses_shared_strings) = determine_sheet_value_mode(xml, &style_profile);
    assert_eq!(mode, SheetValueMode::RawXmlStreaming);
    assert!(!uses_shared_strings);
}

#[test]
fn determine_sheet_value_mode_allows_non_date_styled_numeric_cells() {
    let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1">
      <c r="A1" s="3"><v>123.45</v></c>
    </row>
  </sheetData>
</worksheet>"#;

    let (mode, uses_shared_strings) =
        determine_sheet_value_mode(xml, &WorkbookStyleProfile::default());
    assert_eq!(mode, SheetValueMode::RawXmlStreaming);
    assert!(!uses_shared_strings);
}

#[test]
fn load_shared_strings_concatenates_rich_text_runs() {
    let buf = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buf);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    zip.start_file("xl/sharedStrings.xml", options).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <si><r><t>Hello</t></r><r><t xml:space="preserve"> world</t></r></si>
</sst>"#,
    )
    .unwrap();

    let data = zip.finish().unwrap().into_inner();
    let mut archive = zip::ZipArchive::new(Cursor::new(data.as_slice())).unwrap();
    let strings = load_shared_strings(&mut archive).unwrap();

    assert_eq!(strings, vec!["Hello world".to_string()]);
}

#[test]
fn parse_style_profile_marks_date_styles() {
    let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <numFmts count="1">
    <numFmt numFmtId="165" formatCode="yyyy-mm-dd"/>
  </numFmts>
  <cellXfs count="2">
    <xf numFmtId="0"/>
    <xf numFmtId="165"/>
  </cellXfs>
</styleSheet>"#;

    let profile = parse_style_profile(xml).unwrap();
    assert!(profile.is_date_style(Some(1)));
    assert!(!profile.is_date_style(Some(0)));
}

#[test]
fn comment_bounds_is_zero_zero_when_no_comments() {
    // The no-comment case must be a no-op against data bounds — any
    // non-zero return here would spuriously grow the grid for every
    // sheet in the workbook.
    let metadata = SheetNativeMetadata::default();
    assert_eq!(comment_bounds(&metadata), (0, 0));
}

#[test]
fn comment_bounds_returns_exclusive_bounds_of_comment_cells() {
    // Cells are zero-indexed; bounds are exclusive (row_count, col_count).
    // B2 is (row=1, col=1) and D5 is (row=4, col=3), so the grid must
    // extend to (5, 4) to cover both without panicking the offset lookups
    // in `SheetContext`.
    let mut metadata = SheetNativeMetadata::default();
    let mut comments = BTreeMap::new();
    comments.insert((1, 1), "note on B2".to_string());
    comments.insert((4, 3), "note on D5".to_string());
    metadata.comments = comments;

    assert_eq!(comment_bounds(&metadata), (5, 4));
}

#[test]
fn scan_raw_sheet_profile_expands_bounds_to_cover_comment_only_cells() {
    // Only A1 appears in the XML; the comment is attached to C3, which
    // has no `<c>` element at all. Without bound expansion, `SheetContext`
    // would reject (row=2, col=2) and the annotation would be lost.
    let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1">
      <c r="A1" t="inlineStr"><is><t>alpha</t></is></c>
    </row>
  </sheetData>
</worksheet>"#;

    let mut metadata = SheetNativeMetadata::default();
    let mut comments = BTreeMap::new();
    comments.insert((2, 2), "note on C3".to_string());
    metadata.comments = comments;

    let profile = scan_raw_sheet_profile(xml, &metadata).unwrap();

    assert!(
        profile.total_rows >= 3,
        "grid must include commented row 2 (got {} rows)",
        profile.total_rows
    );
    assert!(
        profile.total_cols >= 3,
        "grid must include commented col 2 (got {} cols)",
        profile.total_cols
    );
}

#[test]
fn scan_raw_sheet_profile_no_comments_preserves_prior_bounds() {
    // Non-regressive: sheets without comments keep data-only bounds
    // exactly — expansion is conditional on commented cells existing.
    let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>x</t></is></c></row>
    <row r="2"><c r="B2"><v>42</v></c></row>
  </sheetData>
</worksheet>"#;

    let metadata = SheetNativeMetadata::default();
    let profile = scan_raw_sheet_profile(xml, &metadata).unwrap();

    // A1 + B2 → exclusive bounds (2, 2).
    assert_eq!(profile.total_rows, 2);
    assert_eq!(profile.total_cols, 2);
}

// Date serialisation is covered end-to-end by the POI-aligned corpus in
// `number_format::date_formatter::tests` (`format_date_body` +
// `format_elapsed_body`). No dedicated `excel_serial_to_iso` helper
// remains: the POI-aligned pipeline is the sole date renderer.
