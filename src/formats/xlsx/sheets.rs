//! Sheet-level processing: cell iteration, native SpreadsheetML metadata parsing,
//! and primitive emission.

use std::io::Cursor;

use calamine::{Reader, Xlsx};

use crate::error::{IdpError, IdpResult, Warning, WarningKind};
use crate::pipeline::PrimitiveSink;

use self::emit::{process_sheet_raw, process_sheet_with_workbook};
use self::metadata::{
    parse_relationship_targets, parse_sheet_native_metadata, parse_workbook_sheet_descriptors,
    read_zip_entry_raw, resolve_relationship_target,
};
use self::raw::determine_sheet_value_mode;
use super::types::SheetNativeMetadata;
use super::workbook_support::{
    WorkbookStyleProfile, load_shared_strings, load_workbook_style_profile,
    parse_workbook_date_1904,
};

mod emit;
mod metadata;
mod raw;

#[derive(Debug, Clone)]
struct SheetDescriptor {
    name: String,
    relationship_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SheetValueMode {
    RawXmlStreaming,
    WorkbookDecoder,
}

#[derive(Debug, Clone)]
struct SheetProcessingEntry {
    name: String,
    path: Option<String>,
    native_metadata: SheetNativeMetadata,
    value_mode: SheetValueMode,
    uses_shared_strings: bool,
}

#[derive(Debug, Default)]
struct SheetProcessingPlan {
    sheets: Vec<SheetProcessingEntry>,
    date_1904: bool,
    style_profile: WorkbookStyleProfile,
}

#[derive(Debug, Clone)]
struct PendingSheetHyperlink {
    start_row: u32,
    start_col: u32,
    end_row: u32,
    end_col: u32,
    relationship_id: Option<String>,
    location: Option<String>,
}

struct RawSheetDecodeContext<'a> {
    page: u32,
    shared_strings: &'a [String],
    date_1904: bool,
    style_profile: &'a WorkbookStyleProfile,
    warnings: &'a mut Vec<Warning>,
    source_order: &'a mut u64,
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Process all sheets and emit Primitives.
pub(crate) fn process_sheets(
    data: &[u8],
    primitives: &mut impl PrimitiveSink,
    warnings: &mut Vec<Warning>,
) -> IdpResult<(u32, Option<String>)> {
    let processing_plan = build_sheet_processing_plan(data, warnings);
    let mut source_order: u64 = 0;
    let has_streaming_sheets = processing_plan
        .sheets
        .iter()
        .any(|sheet| matches!(sheet.value_mode, SheetValueMode::RawXmlStreaming));
    let needs_workbook_decoder = processing_plan.sheets.is_empty()
        || processing_plan
            .sheets
            .iter()
            .any(|sheet| matches!(sheet.value_mode, SheetValueMode::WorkbookDecoder));

    let mut workbook: Option<Xlsx<_>> = if needs_workbook_decoder {
        Some(
            Xlsx::new(Cursor::new(data))
                .map_err(|e| IdpError::UnsupportedFormat(format!("Not a valid XLSX: {e}")))?,
        )
    } else {
        None
    };

    if processing_plan.sheets.is_empty() {
        let workbook = workbook.as_mut().ok_or_else(|| IdpError::Decode {
            context: "xlsx sheet processing".to_string(),
            message: "Workbook decoder missing for fallback XLSX processing".to_string(),
        })?;
        let sheet_names = workbook.sheet_names().to_vec();
        let sheet_count = sheet_names.len() as u32;

        for (page, sheet_name) in sheet_names.iter().enumerate() {
            let sheet = SheetProcessingEntry {
                name: sheet_name.clone(),
                path: None,
                native_metadata: SheetNativeMetadata::default(),
                value_mode: SheetValueMode::WorkbookDecoder,
                uses_shared_strings: false,
            };
            process_sheet_with_workbook(
                workbook,
                &sheet,
                page as u32,
                primitives,
                warnings,
                &mut source_order,
            );
        }

        return Ok((sheet_count, None));
    }

    let mut raw_archive = if has_streaming_sheets {
        Some(
            zip::ZipArchive::new(Cursor::new(data))
                .map_err(|e| IdpError::UnsupportedFormat(format!("Not a valid XLSX ZIP: {e}")))?,
        )
    } else {
        None
    };

    let any_streaming_shared_strings = processing_plan.sheets.iter().any(|sheet| {
        sheet.uses_shared_strings && matches!(sheet.value_mode, SheetValueMode::RawXmlStreaming)
    });
    let shared_strings = if any_streaming_shared_strings {
        let archive = raw_archive.as_mut().ok_or_else(|| IdpError::Decode {
            context: "xlsx shared strings loading".to_string(),
            message: "Raw XLSX archive missing while streaming sheets require shared strings"
                .to_string(),
        })?;
        load_shared_strings(archive).ok()
    } else {
        None
    };

    let sheet_count = processing_plan.sheets.len() as u32;
    for (page, sheet) in processing_plan.sheets.iter().enumerate() {
        let page = page as u32;
        let used_streaming = if matches!(sheet.value_mode, SheetValueMode::RawXmlStreaming)
            && sheet.path.is_some()
            && raw_archive.is_some()
            && (!sheet.uses_shared_strings || shared_strings.is_some())
        {
            let mut buffered = Vec::new();
            let mut tentative_source_order = source_order;
            let streamed = raw_archive.as_mut().is_some_and(|archive| {
                let mut raw_ctx = RawSheetDecodeContext {
                    page,
                    shared_strings: shared_strings.as_deref().unwrap_or(&[]),
                    date_1904: processing_plan.date_1904,
                    style_profile: &processing_plan.style_profile,
                    warnings,
                    source_order: &mut tentative_source_order,
                };
                process_sheet_raw(archive, sheet, &mut raw_ctx, &mut buffered).is_ok()
            });
            if streamed {
                source_order = tentative_source_order;
                for primitive in buffered {
                    primitives.emit(primitive);
                }
            }
            streamed
        } else {
            false
        };

        if used_streaming {
            continue;
        }

        if let Some(workbook) = workbook.as_mut() {
            process_sheet_with_workbook(
                workbook,
                sheet,
                page,
                primitives,
                warnings,
                &mut source_order,
            );
        } else {
            return Err(IdpError::Decode {
                context: "xlsx sheet processing".to_string(),
                message: format!(
                    "Sheet '{}' required workbook-decoder fallback, but no workbook decoder was available",
                    sheet.name
                ),
            });
        }
    }

    Ok((sheet_count, None))
}

pub(crate) fn inspect_workbook_sheet_count(data: &[u8]) -> IdpResult<u32> {
    let cursor = Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| IdpError::UnsupportedFormat(format!("Not a valid XLSX ZIP: {e}")))?;

    let workbook_xml = read_zip_entry_raw(&mut archive, "xl/workbook.xml", None, &mut Vec::new())
        .ok_or_else(|| IdpError::MissingPart("xl/workbook.xml".to_string()))?;
    let sheets = parse_workbook_sheet_descriptors(&workbook_xml).map_err(|e| IdpError::Decode {
        context: "xlsx workbook inspection".to_string(),
        message: format!("Failed to parse xl/workbook.xml: {e}"),
    })?;

    Ok(sheets.len() as u32)
}

// ---------------------------------------------------------------------------
// Native SpreadsheetML metadata parsing from raw ZIP
// ---------------------------------------------------------------------------

fn build_sheet_processing_plan(data: &[u8], warnings: &mut Vec<Warning>) -> SheetProcessingPlan {
    let mut plan = SheetProcessingPlan::default();
    let cursor = Cursor::new(data);
    let mut archive = match zip::ZipArchive::new(cursor) {
        Ok(archive) => archive,
        Err(e) => {
            warnings.push(Warning {
                kind: WarningKind::PartialExtraction,
                message: format!(
                    "Failed to reopen XLSX ZIP for sheet metadata parsing: {}",
                    e
                ),
                page: None,
            });
            return plan;
        }
    };

    let workbook_xml = match read_zip_entry_raw(&mut archive, "xl/workbook.xml", None, warnings) {
        Some(xml) => xml,
        None => return plan,
    };
    plan.date_1904 = parse_workbook_date_1904(&workbook_xml);
    let workbook_rels =
        match read_zip_entry_raw(&mut archive, "xl/_rels/workbook.xml.rels", None, warnings) {
            Some(xml) => xml,
            None => return plan,
        };
    plan.style_profile = load_workbook_style_profile(&mut archive, warnings).unwrap_or_default();

    let sheets = match parse_workbook_sheet_descriptors(&workbook_xml) {
        Ok(sheets) => sheets,
        Err(e) => {
            warnings.push(Warning {
                kind: WarningKind::MalformedContent,
                message: format!("Failed to parse xl/workbook.xml: {}", e),
                page: None,
            });
            return plan;
        }
    };

    let workbook_targets = match parse_relationship_targets(&workbook_rels, Some("worksheet")) {
        Ok(targets) => targets,
        Err(e) => {
            warnings.push(Warning {
                kind: WarningKind::MalformedContent,
                message: format!("Failed to parse xl/_rels/workbook.xml.rels: {}", e),
                page: None,
            });
            return plan;
        }
    };

    for (page, sheet) in sheets.iter().enumerate() {
        let mut entry = SheetProcessingEntry {
            name: sheet.name.clone(),
            path: None,
            native_metadata: SheetNativeMetadata::default(),
            value_mode: SheetValueMode::WorkbookDecoder,
            uses_shared_strings: false,
        };

        let Some(target) = workbook_targets.get(&sheet.relationship_id) else {
            warnings.push(Warning {
                kind: WarningKind::UnresolvedRelationship,
                message: format!(
                    "Worksheet relationship '{}' for sheet '{}' could not be resolved",
                    sheet.relationship_id, sheet.name
                ),
                page: Some(page as u32),
            });
            plan.sheets.push(entry);
            continue;
        };

        let sheet_path = resolve_relationship_target("xl/workbook.xml", target);
        entry.path = Some(sheet_path.clone());
        let sheet_xml =
            match read_zip_entry_raw(&mut archive, &sheet_path, Some(page as u32), warnings) {
                Some(xml) => xml,
                None => {
                    plan.sheets.push(entry);
                    continue;
                }
            };

        entry.native_metadata = parse_sheet_native_metadata(
            &mut archive,
            &sheet.name,
            &sheet_path,
            &sheet_xml,
            page as u32,
            warnings,
        );
        let (value_mode, uses_shared_strings) =
            determine_sheet_value_mode(&sheet_xml, &plan.style_profile);
        entry.value_mode = value_mode;
        entry.uses_shared_strings = uses_shared_strings;
        plan.sheets.push(entry);
    }

    plan
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
