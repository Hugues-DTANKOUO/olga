use std::collections::HashMap;
use std::io::{Cursor, Read};

use quick_xml::events::Event;
use quick_xml::reader::Reader as XmlReader;
use unicode_normalization::UnicodeNormalization;

use crate::error::{IdpError, IdpResult, Warning, WarningKind};
use crate::formats::xml_utils::{attr_value, local_name, resolve_predefined_entity};

use super::number_format::{CellNumberFormat, parse_excel_number_format};

/// Per-workbook style profile.
///
/// Excel's display pipeline is two-stage: each cell carries a style index
/// (`s` attribute on `<c>`) that points into `cellXfs` in `xl/styles.xml`;
/// each `xf` entry's `numFmtId` resolves to either a built-in format
/// (ECMA-376 § 18.8.30, IDs 0-49) or a custom format declared in
/// `<numFmts>`. The format string is what turns a raw numeric value into
/// what the user sees (`46115` → `2026-04-03`, `-0.0261` → `(2.6%)`).
///
/// `formats[xf_index]` is the pre-resolved [`CellNumberFormat`] for each
/// cell-xf entry, built once at load time. This is the decoder-side
/// equivalent of what openpyxl / pandas do on the Python side: keep the
/// classifier output beside the cell so the reader only sees a clean
/// category + numeric knobs, no format-string re-parsing per cell.
///
/// `format_codes[xf_index]` is the raw format-code string resolved for
/// the same cell-xf entry (either the built-in canonical string or the
/// declared custom `<numFmt formatCode=...>`). Kept in parallel with
/// `formats` because the POI-aligned engine consumes the raw code string
/// rather than the classifier's category: grammar features the legacy
/// classifier collapses to `Default` (scientific, literal suffix, custom
/// sign-bearing sections) still have well-defined POI renderings, and
/// those renderings depend on the *exact* format string, not a bucket.
/// `None` means the xf's numFmtId was `0` / missing / reserved — POI's
/// General formatter applies.
#[derive(Debug, Clone, Default)]
pub(crate) struct WorkbookStyleProfile {
    /// Resolved format classification indexed by cellXfs entry index.
    formats: Vec<CellNumberFormat>,
    /// Raw format-code string indexed by cellXfs entry index. Parallel
    /// to `formats`.
    format_codes: Vec<Option<String>>,
}

impl WorkbookStyleProfile {
    /// Resolve the format classification for a cell's style index.
    ///
    /// Unknown / out-of-range indices fall back to [`CellNumberFormat::Default`],
    /// which the renderer treats as "emit the raw numeric representation"
    /// — i.e. strictly non-regressive against the pre-format-preservation
    /// baseline.
    pub(crate) fn format_for_style(&self, style_idx: Option<u32>) -> CellNumberFormat {
        style_idx
            .and_then(|idx| self.formats.get(idx as usize))
            .cloned()
            .unwrap_or(CellNumberFormat::Default)
    }

    /// Resolve the raw format-code string for a cell's style index.
    ///
    /// The POI-aligned engine consumes this to pick up section-structure,
    /// literal runs, and exponent grammar the classifier loses. Returns
    /// `None` when the xf has no recognised numFmtId (built-in `0` /
    /// unknown / reserved / missing style index) — the caller falls back
    /// to General.
    pub(crate) fn format_code_for_style(&self, style_idx: Option<u32>) -> Option<&str> {
        style_idx
            .and_then(|idx| self.format_codes.get(idx as usize))
            .and_then(|opt| opt.as_deref())
    }

    #[cfg(test)]
    pub(crate) fn is_date_style(&self, style_idx: Option<u32>) -> bool {
        matches!(self.format_for_style(style_idx), CellNumberFormat::Date)
    }
}

pub(crate) fn load_shared_strings(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
) -> IdpResult<Vec<String>> {
    let mut entry = match archive.by_name("xl/sharedStrings.xml") {
        Ok(entry) => entry,
        Err(zip::result::ZipError::FileNotFound) => return Ok(Vec::new()),
        Err(err) => {
            return Err(IdpError::Decode {
                context: "xlsx shared strings".to_string(),
                message: err.to_string(),
            });
        }
    };

    let mut xml = Vec::with_capacity(entry.size() as usize);
    entry
        .read_to_end(&mut xml)
        .map_err(|err| IdpError::Decode {
            context: "xlsx shared strings".to_string(),
            message: format!("Failed to read xl/sharedStrings.xml: {err}"),
        })?;

    let mut reader = XmlReader::from_reader(xml.as_slice());
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut strings = Vec::new();
    let mut in_si = false;
    let mut current = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if local_name(e.name().as_ref()) == b"si" => {
                in_si = true;
                current.clear();
            }
            Ok(Event::Text(ref e)) if in_si => {
                current.push_str(&String::from_utf8_lossy(e.as_ref()));
            }
            Ok(Event::GeneralRef(ref e)) if in_si => {
                let ch = if let Ok(Some(c)) = e.resolve_char_ref() {
                    Some(c)
                } else {
                    let name = String::from_utf8_lossy(e.as_ref());
                    resolve_predefined_entity(&name)
                };
                if let Some(c) = ch {
                    current.push(c);
                }
            }
            Ok(Event::End(ref e)) if local_name(e.name().as_ref()) == b"si" => {
                strings.push(current.nfc().collect());
                current.clear();
                in_si = false;
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(IdpError::Decode {
                    context: "xlsx shared strings".to_string(),
                    message: format!("Failed to parse xl/sharedStrings.xml: {err}"),
                });
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(strings)
}

pub(crate) fn parse_workbook_date_1904(xml: &[u8]) -> bool {
    let mut reader = XmlReader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e))
                if local_name(e.name().as_ref()) == b"workbookPr" =>
            {
                for attr in e.attributes().flatten() {
                    if local_name(attr.key.as_ref()) == b"date1904" {
                        return parse_xml_bool(&attr_value(&attr));
                    }
                }
                return false;
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    false
}

pub(crate) fn load_workbook_style_profile(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    warnings: &mut Vec<Warning>,
) -> IdpResult<WorkbookStyleProfile> {
    let mut entry = match archive.by_name("xl/styles.xml") {
        Ok(entry) => entry,
        Err(zip::result::ZipError::FileNotFound) => return Ok(WorkbookStyleProfile::default()),
        Err(err) => {
            return Err(IdpError::Decode {
                context: "xlsx styles".to_string(),
                message: err.to_string(),
            });
        }
    };
    let mut xml = Vec::with_capacity(entry.size() as usize);
    entry
        .read_to_end(&mut xml)
        .map_err(|err| IdpError::Decode {
            context: "xlsx styles".to_string(),
            message: format!("Failed to read xl/styles.xml: {err}"),
        })?;

    parse_style_profile(&xml).map_err(|err| {
        warnings.push(Warning {
            kind: WarningKind::MalformedContent,
            message: format!("Failed to parse xl/styles.xml: {err}"),
            page: None,
        });
        IdpError::Decode {
            context: "xlsx styles".to_string(),
            message: err.to_string(),
        }
    })
}

pub(crate) fn parse_style_profile(xml: &[u8]) -> Result<WorkbookStyleProfile, quick_xml::Error> {
    let mut reader = XmlReader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut profile = WorkbookStyleProfile::default();
    let mut custom_num_formats: HashMap<u32, String> = HashMap::new();
    let mut in_cell_xfs = false;

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(ref e) if local_name(e.name().as_ref()) == b"cellXfs" => {
                in_cell_xfs = true;
                profile.formats.clear();
                profile.format_codes.clear();
            }
            Event::End(ref e) if local_name(e.name().as_ref()) == b"cellXfs" => {
                in_cell_xfs = false;
            }
            Event::Empty(ref e) | Event::Start(ref e)
                if local_name(e.name().as_ref()) == b"numFmt" =>
            {
                let mut num_fmt_id = None;
                let mut format_code = None;
                for attr in e.attributes().flatten() {
                    match local_name(attr.key.as_ref()).as_slice() {
                        b"numFmtId" => num_fmt_id = attr_value(&attr).parse::<u32>().ok(),
                        b"formatCode" => {
                            // formatCode commonly contains XML-escaped quoted
                            // literals (e.g. `&quot;$&quot;#,##0`). Decode the
                            // five predefined entities so the format-code
                            // parser sees the canonical Excel form.
                            format_code = Some(unescape_xml_entities(&attr_value(&attr)));
                        }
                        _ => {}
                    }
                }
                if let (Some(num_fmt_id), Some(format_code)) = (num_fmt_id, format_code) {
                    custom_num_formats.insert(num_fmt_id, format_code);
                }
            }
            Event::Empty(ref e) | Event::Start(ref e)
                if in_cell_xfs && local_name(e.name().as_ref()) == b"xf" =>
            {
                let num_fmt_id = e
                    .attributes()
                    .flatten()
                    .find(|attr| local_name(attr.key.as_ref()) == b"numFmtId")
                    .and_then(|attr| attr_value(&attr).parse::<u32>().ok());
                let format = resolve_num_fmt(num_fmt_id, &custom_num_formats);
                let format_code = resolve_num_fmt_code(num_fmt_id, &custom_num_formats);
                profile.formats.push(format);
                profile.format_codes.push(format_code);
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(profile)
}

/// Resolve a numFmtId to its [`CellNumberFormat`] classification.
///
/// Resolution order, following ECMA-376 § 18.8.30:
///
/// 1. `None` or `0`          → `Default` (General).
/// 2. Custom (id in numFmts) → parse the declared formatCode string.
/// 3. Built-in (ids 0-49)    → use the canonical format string from the
///    spec, then parse.
/// 4. Otherwise              → `Default` (unknown / reserved).
fn resolve_num_fmt(num_fmt_id: Option<u32>, custom: &HashMap<u32, String>) -> CellNumberFormat {
    let Some(id) = num_fmt_id else {
        return CellNumberFormat::Default;
    };
    if let Some(code) = custom.get(&id) {
        return parse_excel_number_format(code);
    }
    match builtin_num_fmt_code(id) {
        Some(code) => parse_excel_number_format(code),
        None => CellNumberFormat::Default,
    }
}

/// Resolve a numFmtId to its raw format-code string.
///
/// Mirrors the resolution order of [`resolve_num_fmt`] but returns the
/// pre-classification format string so the POI-aligned engine has the
/// section structure, literal runs, and exponent grammar the classifier
/// loses. Returns `None` for `numFmtId 0` (the General sentinel) and for
/// reserved / unknown ids — the caller treats `None` as General.
fn resolve_num_fmt_code(num_fmt_id: Option<u32>, custom: &HashMap<u32, String>) -> Option<String> {
    let id = num_fmt_id?;
    if let Some(code) = custom.get(&id) {
        return Some(code.clone());
    }
    builtin_num_fmt_code(id).map(str::to_string)
}

/// Canonical format code for ECMA-376 built-in numFmtIds.
///
/// The spec (§ 18.8.30) fixes these for the en-US locale; in practice all
/// implementations treat them as the source of truth regardless of locale
/// because they are the only ids that are *implicit* — not declared in
/// `<numFmts>`.
///
/// IDs not listed here (5-8, 23-36, 41-44) are reserved in ECMA-376 and
/// have no portable canonical representation — authoring tools re-export
/// them as custom numFmts when used. We classify as `Default` rather than
/// guess.
fn builtin_num_fmt_code(id: u32) -> Option<&'static str> {
    match id {
        0 => Some("General"),
        1 => Some("0"),
        2 => Some("0.00"),
        3 => Some("#,##0"),
        4 => Some("#,##0.00"),
        9 => Some("0%"),
        10 => Some("0.00%"),
        11 => Some("0.00E+00"),
        12 => Some("# ?/?"),
        13 => Some("# ??/??"),
        14 => Some("m/d/yyyy"),
        15 => Some("d-mmm-yy"),
        16 => Some("d-mmm"),
        17 => Some("mmm-yy"),
        18 => Some("h:mm AM/PM"),
        19 => Some("h:mm:ss AM/PM"),
        20 => Some("h:mm"),
        21 => Some("h:mm:ss"),
        22 => Some("m/d/yyyy h:mm"),
        37 => Some("#,##0 ;(#,##0)"),
        38 => Some("#,##0 ;[Red](#,##0)"),
        39 => Some("#,##0.00;(#,##0.00)"),
        40 => Some("#,##0.00;[Red](#,##0.00)"),
        45 => Some("mm:ss"),
        46 => Some("[h]:mm:ss"),
        47 => Some("mmss.0"),
        48 => Some("##0.0E+0"),
        49 => Some("@"),
        _ => None,
    }
}

fn parse_xml_bool(value: &str) -> bool {
    matches!(value, "1" | "true" | "TRUE" | "True")
}

/// Decode the five XML predefined entities (`&amp;`, `&lt;`, `&gt;`,
/// `&apos;`, `&quot;`) in-place. quick-xml's raw attribute value keeps
/// them as-is, but a format-code parser needs the canonical characters
/// (especially `&quot;` → `"` for quoted currency literals).
fn unescape_xml_entities(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'&'
            && let Some(semi) = bytes[i + 1..].iter().position(|&b| b == b';')
        {
            let name = &input[i + 1..i + 1 + semi];
            if let Some(ch) = resolve_predefined_entity(name) {
                out.push(ch);
                i += semi + 2;
                continue;
            }
        }
        // Not a recognised entity — pass through as-is.
        let ch = input[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_date_style_resolves_to_date() {
        // cellXfs: a single xf with numFmtId="14" (built-in m/d/yyyy)
        let xml = br#"<?xml version="1.0"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <cellXfs count="1">
    <xf numFmtId="14"/>
  </cellXfs>
</styleSheet>"#;
        let profile = parse_style_profile(xml).unwrap();
        assert_eq!(profile.format_for_style(Some(0)), CellNumberFormat::Date);
        assert!(profile.is_date_style(Some(0)));
    }

    #[test]
    fn builtin_currency_style_resolves_to_currency() {
        // numFmtId 37 → #,##0 ;(#,##0) — accounting integer
        let xml = br#"<?xml version="1.0"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <cellXfs count="1">
    <xf numFmtId="37"/>
  </cellXfs>
</styleSheet>"#;
        let profile = parse_style_profile(xml).unwrap();
        let fmt = profile.format_for_style(Some(0));
        assert_eq!(
            fmt,
            CellNumberFormat::Number {
                decimals: 0,
                thousands_sep: true,
            }
        );
    }

    #[test]
    fn builtin_percent_style_resolves_to_percent() {
        // numFmtId 10 → 0.00%
        let xml = br#"<?xml version="1.0"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <cellXfs count="1">
    <xf numFmtId="10"/>
  </cellXfs>
</styleSheet>"#;
        let profile = parse_style_profile(xml).unwrap();
        assert_eq!(
            profile.format_for_style(Some(0)),
            CellNumberFormat::Percent { decimals: 2 }
        );
    }

    #[test]
    fn custom_num_fmt_resolves() {
        // Declare a custom $-currency fmt at id 164 and reference it.
        let xml = br#"<?xml version="1.0"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <numFmts count="1">
    <numFmt numFmtId="164" formatCode="&quot;$&quot;#,##0"/>
  </numFmts>
  <cellXfs count="1">
    <xf numFmtId="164"/>
  </cellXfs>
</styleSheet>"#;
        let profile = parse_style_profile(xml).unwrap();
        assert_eq!(
            profile.format_for_style(Some(0)),
            CellNumberFormat::Currency {
                prefix: "$".to_string(),
                suffix: String::new(),
                decimals: 0,
                thousands_sep: true,
                negative_parens: false,
            }
        );
    }

    #[test]
    fn unknown_and_missing_indices_fall_through_to_default() {
        let xml = br#"<?xml version="1.0"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <cellXfs count="1">
    <xf numFmtId="0"/>
  </cellXfs>
</styleSheet>"#;
        let profile = parse_style_profile(xml).unwrap();
        // Default numFmtId 0 → General → Default classification
        assert_eq!(profile.format_for_style(Some(0)), CellNumberFormat::Default);
        // Out-of-range index → Default (non-regressive)
        assert_eq!(
            profile.format_for_style(Some(99)),
            CellNumberFormat::Default
        );
        // No style → Default
        assert_eq!(profile.format_for_style(None), CellNumberFormat::Default);
    }

    #[test]
    fn reserved_builtin_ids_fall_through_to_default() {
        // numFmtId 5-8 are reserved (currency-like) but have no portable
        // canonical string; classify as Default.
        let xml = br#"<?xml version="1.0"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <cellXfs count="1">
    <xf numFmtId="5"/>
  </cellXfs>
</styleSheet>"#;
        let profile = parse_style_profile(xml).unwrap();
        assert_eq!(profile.format_for_style(Some(0)), CellNumberFormat::Default);
    }
}
