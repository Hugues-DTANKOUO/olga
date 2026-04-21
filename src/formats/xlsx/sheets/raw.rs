use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader as XmlReader;

use crate::error::{IdpError, IdpResult, Warning, WarningKind};
use crate::formats::xml_utils::{attr_value, local_name, resolve_predefined_entity};

use super::super::number_format::{
    CellNumberFormat, FormatOptions, format_value, push_format_warnings_into,
};
use super::super::types::SheetNativeMetadata;
use super::super::workbook_support::WorkbookStyleProfile;
use super::metadata::parse_cell_ref;
use super::{RawSheetDecodeContext, SheetValueMode};

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct RawSheetProfile {
    pub(super) total_rows: u32,
    pub(super) total_cols: u32,
    pub(super) heuristic_header_row: Option<u32>,
}

pub(super) fn scan_raw_sheet_profile(
    xml: &[u8],
    metadata: &SheetNativeMetadata,
) -> IdpResult<RawSheetProfile> {
    let mut reader = XmlReader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut profile = RawSheetProfile::default();
    let mut current_row = 0u32;
    let mut current_col = 0u32;
    // Hidden rows are UI-visibility metadata only (see SheetContext::new); the
    // header heuristic therefore considers row 0 directly, matching pandas /
    // openpyxl / calamine semantics where hidden cells remain part of the
    // logical workbook.
    let mut first_row_any_string = false;
    let mut first_row_all_strings_or_empty = true;
    let mut has_following_row = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if local_name(e.name().as_ref()) == b"row" => {
                current_row = row_index_from_attrs(e, current_row);
                current_col = 0;
                profile.total_rows = profile.total_rows.max(current_row.saturating_add(1));

                if current_row > 0 {
                    has_following_row = true;
                }
            }
            Ok(Event::End(ref e)) if local_name(e.name().as_ref()) == b"row" => {
                current_row = current_row.saturating_add(1);
                current_col = 0;
            }
            Ok(Event::Start(ref e)) if local_name(e.name().as_ref()) == b"c" => {
                let (row_idx, col_idx) = cell_ref_from_attrs(e, current_row, current_col)?;
                current_row = row_idx;
                current_col = col_idx;
                profile.total_rows = profile.total_rows.max(row_idx.saturating_add(1));
                profile.total_cols = profile.total_cols.max(col_idx.saturating_add(1));

                if row_idx == 0 {
                    match cell_type_from_attrs(e).as_deref() {
                        Some("s" | "str" | "inlineStr") => first_row_any_string = true,
                        Some(_) | None => first_row_all_strings_or_empty = false,
                    }
                }

                skip_cell_to_end(&mut reader)?;
                current_col = current_col.saturating_add(1);
            }
            Ok(Event::Empty(ref e)) if local_name(e.name().as_ref()) == b"c" => {
                let (row_idx, col_idx) = cell_ref_from_attrs(e, current_row, current_col)?;
                current_row = row_idx;
                current_col = col_idx.saturating_add(1);
                profile.total_rows = profile.total_rows.max(row_idx.saturating_add(1));
                profile.total_cols = profile.total_cols.max(col_idx.saturating_add(1));

                if row_idx == 0 {
                    match cell_type_from_attrs(e).as_deref() {
                        Some("s" | "str" | "inlineStr") => first_row_any_string = true,
                        Some(_) | None => first_row_all_strings_or_empty = false,
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(IdpError::Decode {
                    context: "xlsx raw sheet profile".to_string(),
                    message: e.to_string(),
                });
            }
            _ => {}
        }
        buf.clear();
    }

    // A comment attached to an otherwise-empty cell is still authored
    // content (annotation-only cells are the whole point of the comments
    // feature in Excel — "Insert Comment" on a blank cell). Make sure the
    // grid bounds cover every commented cell, not just the `<c>`-element-
    // backed ones, so `SheetContext` accepts the commented (row, col)
    // pairs the emit pass will later revisit as comment-only cells.
    let (comment_row_bound, comment_col_bound) = comment_bounds(metadata);
    profile.total_rows = profile.total_rows.max(comment_row_bound);
    profile.total_cols = profile.total_cols.max(comment_col_bound);

    if metadata.native_tables.is_empty()
        && has_following_row
        && first_row_all_strings_or_empty
        && first_row_any_string
    {
        profile.heuristic_header_row = Some(0);
    }

    Ok(profile)
}

/// Compute the exclusive bounds `(row_count, col_count)` required to fit
/// every commented cell declared in the sheet's native metadata.
///
/// Returns `(0, 0)` when there are no comments, so `.max(...)` against the
/// data-derived bounds is a no-op on the common case. On a sheet with any
/// comment, this returns `(max_row + 1, max_col + 1)` — bounds big enough
/// that every commented cell is a valid index into `SheetContext`'s
/// identity-mapped offset tables.
pub(super) fn comment_bounds(metadata: &SheetNativeMetadata) -> (u32, u32) {
    let mut row_bound = 0u32;
    let mut col_bound = 0u32;
    for &(row, col) in metadata.comments.keys() {
        row_bound = row_bound.max(row.saturating_add(1));
        col_bound = col_bound.max(col.saturating_add(1));
    }
    (row_bound, col_bound)
}

pub(super) fn determine_sheet_value_mode(
    xml: &[u8],
    style_profile: &WorkbookStyleProfile,
) -> (SheetValueMode, bool) {
    let mut reader = XmlReader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut uses_shared_strings = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e))
                if local_name(e.name().as_ref()) == b"c" =>
            {
                let cell_type = cell_type_from_attrs(e);
                let style_idx = cell_style_from_attrs(e);
                if matches!(cell_type.as_deref(), Some("s")) {
                    uses_shared_strings = true;
                }

                if cell_requires_workbook_decoder(cell_type.as_deref(), style_idx, style_profile) {
                    return (SheetValueMode::WorkbookDecoder, uses_shared_strings);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return (SheetValueMode::WorkbookDecoder, uses_shared_strings),
            _ => {}
        }
        buf.clear();
    }

    (SheetValueMode::RawXmlStreaming, uses_shared_strings)
}

pub(super) fn row_index_from_attrs(start: &BytesStart<'_>, fallback_row: u32) -> u32 {
    start
        .attributes()
        .flatten()
        .find(|attr| local_name(attr.key.as_ref()) == b"r")
        .and_then(|attr| attr_value(&attr).parse::<u32>().ok())
        .and_then(|row| row.checked_sub(1))
        .unwrap_or(fallback_row)
}

pub(super) fn cell_ref_from_attrs(
    start: &BytesStart<'_>,
    current_row: u32,
    current_col: u32,
) -> IdpResult<(u32, u32)> {
    start
        .attributes()
        .flatten()
        .find(|attr| local_name(attr.key.as_ref()) == b"r")
        .map(|attr| {
            parse_cell_ref(&attr_value(&attr)).ok_or_else(|| IdpError::Decode {
                context: "xlsx raw cell reference".to_string(),
                message: format!("Malformed cell reference '{}'", attr_value(&attr)),
            })
        })
        .transpose()
        .map(|value| value.unwrap_or((current_row, current_col)))
}

pub(super) fn read_raw_cell_content(
    reader: &mut XmlReader<&[u8]>,
    start: &BytesStart<'_>,
    raw_ctx: &mut RawSheetDecodeContext<'_>,
    sheet_name: &str,
) -> IdpResult<String> {
    let cell_type = cell_type_from_attrs(start);
    let style_idx = cell_style_from_attrs(start);
    let mut buf = Vec::new();
    let mut value_text = String::new();
    let mut inline_text = String::new();
    let mut capture_value = false;
    let mut has_formula = false;
    let mut saw_value_element = false;
    let mut saw_inline_string = false;
    let mut inline_depth = 0usize;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if local_name(e.name().as_ref()) == b"f" => {
                has_formula = true;
            }
            Ok(Event::Start(ref e)) if local_name(e.name().as_ref()) == b"v" => {
                capture_value = true;
                saw_value_element = true;
            }
            Ok(Event::End(ref e)) if local_name(e.name().as_ref()) == b"v" => {
                capture_value = false;
            }
            Ok(Event::Start(ref e)) if local_name(e.name().as_ref()) == b"is" => {
                inline_depth += 1;
                saw_inline_string = true;
            }
            Ok(Event::End(ref e)) if local_name(e.name().as_ref()) == b"is" => {
                inline_depth = inline_depth.saturating_sub(1);
            }
            Ok(Event::Text(ref e)) => {
                let text = String::from_utf8_lossy(e.as_ref());
                if capture_value {
                    value_text.push_str(&text);
                } else if inline_depth > 0 {
                    inline_text.push_str(&text);
                }
            }
            Ok(Event::GeneralRef(ref e)) => {
                let ch = if let Ok(Some(c)) = e.resolve_char_ref() {
                    Some(c)
                } else {
                    let name = String::from_utf8_lossy(e.as_ref());
                    resolve_predefined_entity(&name)
                };
                if let Some(c) = ch {
                    let mut buf_char = [0u8; 4];
                    let s = c.encode_utf8(&mut buf_char);
                    if capture_value {
                        value_text.push_str(s);
                    } else if inline_depth > 0 {
                        inline_text.push_str(s);
                    }
                }
            }
            Ok(Event::End(ref e)) if local_name(e.name().as_ref()) == b"c" => break,
            Ok(Event::Eof) => {
                return Err(IdpError::Decode {
                    context: "xlsx raw sheet cell".to_string(),
                    message: "Unexpected EOF while reading cell".to_string(),
                });
            }
            Err(err) => {
                return Err(IdpError::Decode {
                    context: "xlsx raw sheet cell".to_string(),
                    message: err.to_string(),
                });
            }
            _ => {}
        }
        buf.clear();
    }

    if has_formula && !saw_value_element && !saw_inline_string {
        raw_ctx.warnings.push(Warning {
            kind: WarningKind::PartialExtraction,
            message: format!(
                "Formula cell in sheet '{}' had no cached value on the raw XLSX path; skipped cell content",
                sheet_name
            ),
            page: Some(raw_ctx.page),
        });
        return Ok(String::new());
    }

    Ok(match cell_type.as_deref() {
        Some("s") => {
            let Some(index) = value_text.parse::<usize>().ok() else {
                raw_ctx.warnings.push(Warning {
                    kind: WarningKind::MalformedContent,
                    message: format!(
                        "Shared string index '{}' is malformed in sheet '{}'",
                        value_text, sheet_name
                    ),
                    page: Some(raw_ctx.page),
                });
                return Ok(String::new());
            };
            raw_ctx
                .shared_strings
                .get(index)
                .cloned()
                .unwrap_or_else(|| {
                    raw_ctx.warnings.push(Warning {
                        kind: WarningKind::PartialExtraction,
                        message: format!(
                            "Shared string index {} is out of bounds in sheet '{}'",
                            index, sheet_name
                        ),
                        page: Some(raw_ctx.page),
                    });
                    String::new()
                })
        }
        Some("inlineStr") => inline_text,
        Some("str") | Some("d") => value_text,
        None | Some("n") => {
            // Apply the cell's display format (style-driven) to the raw
            // numeric value through the POI-aligned `format_value`
            // engine. Two short-circuits keep the path strictly
            // non-regressive for unstyled / text-styled cells:
            //
            // * The classifier returns `Default` for cells with no
            //   styled format (no `s=` attribute, or style index
            //   resolves to numFmtId `0`/General) — we keep the raw
            //   textual numeric, same as before the format-preservation
            //   path landed.
            // * The classifier returns `Text` for numFmtId `49` (`@`):
            //   POI's text formatter on a numeric value would render
            //   via General, but the raw stored text (e.g. `"5"` for
            //   the integer 5) is what the user typed and what
            //   downstream consumers expect. Keep the raw text.
            //
            // For everything else (Date / Percent / Currency / Number,
            // plus the grammar the classifier collapses to `Default`
            // like scientific, literal suffix, and custom sign-bearing
            // sections), we resolve the raw format code from the style
            // profile and dispatch through `format_value`. Any
            // degradation surfaced on the returned `FormattedValue` is
            // bridged into the sheet-pipeline's `Warning` channel so
            // telemetry can count it by kind.
            let classification = raw_ctx.style_profile.format_for_style(style_idx);
            match classification {
                CellNumberFormat::Default | CellNumberFormat::Text => value_text,
                _ => match value_text.parse::<f64>().ok() {
                    Some(value) => {
                        let format_code = raw_ctx
                            .style_profile
                            .format_code_for_style(style_idx)
                            .unwrap_or("");
                        let opts = FormatOptions {
                            date_1904: raw_ctx.date_1904,
                        };
                        let fv = format_value(value, format_code, &opts);
                        push_format_warnings_into(&fv, Some(raw_ctx.page), raw_ctx.warnings);
                        fv.text
                    }
                    None => {
                        raw_ctx.warnings.push(Warning {
                            kind: WarningKind::MalformedContent,
                            message: format!(
                                "Styled numeric cell in sheet '{}' carried non-numeric value '{}'",
                                sheet_name, value_text
                            ),
                            page: Some(raw_ctx.page),
                        });
                        value_text
                    }
                },
            }
        }
        Some("b") => {
            if matches!(value_text.as_str(), "1" | "true") {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        Some("e") => {
            if value_text.is_empty() {
                String::new()
            } else {
                format!("#ERR:{}", value_text)
            }
        }
        Some(other) => {
            raw_ctx.warnings.push(Warning {
                kind: WarningKind::PartialExtraction,
                message: format!(
                    "Unsupported raw XLSX cell type '{}' in sheet '{}'; skipped cell content",
                    other, sheet_name
                ),
                page: Some(raw_ctx.page),
            });
            String::new()
        }
    })
}

/// Return `true` when a cell carries a feature the raw XML path cannot
/// decode losslessly — in which case the sheet is routed through calamine's
/// workbook decoder instead.
///
/// Prior to the style-aware raw path, *any* styled numeric (dates) forced
/// the whole sheet to the workbook decoder. That decoder then had to
/// stringify `Data::Float(46115.0)` via `format!("{}", f)` — which loses
/// all format information calamine never captured in the first place
/// ([documented upstream][1]: "no support for reading extra contents such
/// as formatting"). So every format-preserving sheet ended up losing its
/// format, and the budget / currency / percent anomalies tracked to this
/// exact routing.
///
/// The raw path now carries a pre-resolved `CellNumberFormat` for each
/// style index, so styled numerics stay on the raw path and render at
/// extraction time against the style profile. The only reason to escape
/// to calamine now is cell types the raw path really cannot interpret.
///
/// [1]: https://github.com/tafia/calamine#readme
fn cell_requires_workbook_decoder(
    cell_type: Option<&str>,
    _style_idx: Option<u32>,
    _style_profile: &WorkbookStyleProfile,
) -> bool {
    match cell_type {
        None | Some("n" | "s" | "str" | "inlineStr" | "b" | "e" | "d") => false,
        Some(_) => true,
    }
}

fn cell_type_from_attrs(start: &BytesStart<'_>) -> Option<String> {
    start
        .attributes()
        .flatten()
        .find(|attr| local_name(attr.key.as_ref()) == b"t")
        .and_then(|attr| {
            std::str::from_utf8(attr.value.as_ref())
                .ok()
                .map(str::to_owned)
        })
}

fn cell_style_from_attrs(start: &BytesStart<'_>) -> Option<u32> {
    start
        .attributes()
        .flatten()
        .find(|attr| local_name(attr.key.as_ref()) == b"s")
        .and_then(|attr| attr_value(&attr).parse::<u32>().ok())
}

fn skip_cell_to_end(reader: &mut XmlReader<&[u8]>) -> IdpResult<()> {
    let mut depth = 1usize;
    let mut buf = Vec::new();
    while depth > 0 {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if local_name(e.name().as_ref()) == b"c" => depth += 1,
            Ok(Event::End(ref e)) if local_name(e.name().as_ref()) == b"c" => depth -= 1,
            Ok(Event::Eof) => {
                return Err(IdpError::Decode {
                    context: "xlsx raw sheet profile".to_string(),
                    message: "Unexpected EOF while skipping cell".to_string(),
                });
            }
            Err(err) => {
                return Err(IdpError::Decode {
                    context: "xlsx raw sheet profile".to_string(),
                    message: err.to_string(),
                });
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}
