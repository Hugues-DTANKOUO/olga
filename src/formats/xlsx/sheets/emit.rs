use std::collections::HashSet;
use std::io::Cursor;

use calamine::{Data, Range, Reader, Xlsx};
use quick_xml::events::Event;
use quick_xml::reader::Reader as XmlReader;
use unicode_normalization::UnicodeNormalization;

use crate::error::{IdpError, Warning, WarningKind};
use crate::formats::xml_utils::local_name;
use crate::model::*;
use crate::pipeline::PrimitiveSink;

use super::super::types::{NativeTable, SheetContext};
use super::raw::{
    cell_ref_from_attrs, comment_bounds, read_raw_cell_content, row_index_from_attrs,
    scan_raw_sheet_profile,
};
use super::{RawSheetDecodeContext, SheetProcessingEntry};

fn detect_header_row_heuristic(range: &Range<Data>, ctx: &SheetContext) -> Option<u32> {
    if !ctx.native_tables.is_empty() {
        return None;
    }

    let cols: Vec<u32> = (0..ctx.total_cols).collect();
    if cols.is_empty() {
        return None;
    }

    let first_row = 0u32;
    if ctx.total_rows < 2 {
        return None;
    }

    let all_strings_or_empty = cols.iter().all(|col| {
        matches!(
            range.get((first_row as usize, *col as usize)),
            Some(Data::String(_)) | Some(Data::Empty) | None
        )
    });
    let any_string = cols.iter().any(|col| {
        matches!(
            range.get((first_row as usize, *col as usize)),
            Some(Data::String(_))
        )
    });

    (all_strings_or_empty && any_string).then_some(first_row)
}

pub(super) fn process_sheet_with_workbook(
    workbook: &mut Xlsx<Cursor<&[u8]>>,
    sheet: &SheetProcessingEntry,
    page: u32,
    primitives: &mut impl PrimitiveSink,
    warnings: &mut Vec<Warning>,
    source_order: &mut u64,
) {
    let range: Range<Data> = match workbook.worksheet_range(&sheet.name) {
        Ok(r) => r,
        Err(e) => {
            warnings.push(Warning {
                kind: WarningKind::PartialExtraction,
                message: format!("Failed to read sheet '{}': {}", sheet.name, e),
                page: Some(page),
            });
            return;
        }
    };

    // calamine's `Range` is the data-cells bounding box — it does not
    // expand to annotation-only cells. An Excel author can attach a
    // comment to a completely blank cell anywhere on the sheet; if we
    // size the grid from `range.get_size()` alone and that comment falls
    // outside the data box, the comment is silently dropped. Union the
    // data bounds with the bounds required to cover every commented cell.
    let (data_row_count, data_col_count) = range.get_size();
    let (comment_row_bound, comment_col_bound) = comment_bounds(&sheet.native_metadata);
    let row_count = (data_row_count as u32).max(comment_row_bound);
    let col_count = (data_col_count as u32).max(comment_col_bound);

    // After unioning data and comment bounds, a 0-by-0 grid means the
    // sheet has neither data cells nor commented cells — there is nothing
    // to emit. (Either component being non-zero implies the other is too,
    // because comments contribute matched row/col bounds in lockstep.)
    if row_count == 0 || col_count == 0 {
        return;
    }

    let ctx = SheetContext::new(page, row_count, col_count, sheet.native_metadata.clone());
    let heuristic_header_row = detect_header_row_heuristic(&range, &ctx);
    warn_if_heuristic_header_row(&sheet.name, page, heuristic_header_row, warnings);

    emit_sheet_title(page, &sheet.name, primitives, source_order);

    for row_idx in 0..ctx.total_rows {
        for col_idx in 0..ctx.total_cols {
            if ctx.is_merged_non_origin(row_idx, col_idx) {
                continue;
            }

            let content = cell_to_string(range.get((row_idx as usize, col_idx as usize)));
            emit_sheet_cell(
                &ctx,
                row_idx,
                col_idx,
                &content,
                heuristic_header_row,
                primitives,
                source_order,
            );
        }
    }
}

pub(super) fn process_sheet_raw(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    sheet: &SheetProcessingEntry,
    raw_ctx: &mut RawSheetDecodeContext<'_>,
    primitives: &mut impl PrimitiveSink,
) -> Result<(), IdpError> {
    let Some(path) = sheet.path.as_deref() else {
        return Err(IdpError::MissingPart(format!(
            "worksheet path missing for '{}'",
            sheet.name
        )));
    };
    let sheet_xml =
        super::metadata::read_zip_entry_raw(archive, path, Some(raw_ctx.page), raw_ctx.warnings)
            .ok_or_else(|| IdpError::MissingPart(path.to_string()))?;
    let profile = scan_raw_sheet_profile(&sheet_xml, &sheet.native_metadata)?;
    if profile.total_rows == 0 || profile.total_cols == 0 {
        return Ok(());
    }

    let sheet_ctx = SheetContext::new(
        raw_ctx.page,
        profile.total_rows,
        profile.total_cols,
        sheet.native_metadata.clone(),
    );
    warn_if_heuristic_header_row(
        &sheet.name,
        raw_ctx.page,
        profile.heuristic_header_row,
        raw_ctx.warnings,
    );
    emit_sheet_title(raw_ctx.page, &sheet.name, primitives, raw_ctx.source_order);

    let mut reader = XmlReader::from_reader(sheet_xml.as_slice());
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut current_row = 0u32;
    let mut current_col = 0u32;
    // Track every (row, col) the streaming pass emits or intentionally
    // skips. After EOF, any commented cell not in this set is a comment
    // attached to a cell the XML never materialised as a `<c>` element —
    // and that annotation still needs to surface. This is the union-
    // iteration contract documented in `scan_raw_sheet_profile` /
    // `comment_bounds`.
    let mut visited: HashSet<(u32, u32)> = HashSet::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if local_name(e.name().as_ref()) == b"row" => {
                current_row = row_index_from_attrs(e, current_row);
                current_col = 0;
            }
            Ok(Event::End(ref e)) if local_name(e.name().as_ref()) == b"row" => {
                current_row = current_row.saturating_add(1);
                current_col = 0;
            }
            Ok(Event::Start(ref e)) if local_name(e.name().as_ref()) == b"c" => {
                let cell_ref = cell_ref_from_attrs(e, current_row, current_col)?;
                current_row = cell_ref.0;
                current_col = cell_ref.1;
                visited.insert((current_row, current_col));
                let content = read_raw_cell_content(&mut reader, e, raw_ctx, &sheet.name)?;
                emit_sheet_cell(
                    &sheet_ctx,
                    current_row,
                    current_col,
                    &content,
                    profile.heuristic_header_row,
                    primitives,
                    raw_ctx.source_order,
                );
                current_col = current_col.saturating_add(1);
            }
            Ok(Event::Empty(ref e)) if local_name(e.name().as_ref()) == b"c" => {
                let cell_ref = cell_ref_from_attrs(e, current_row, current_col)?;
                current_row = cell_ref.0;
                let col_idx = cell_ref.1;
                current_col = col_idx.saturating_add(1);
                visited.insert((current_row, col_idx));
                // A self-closing `<c/>` carries no inline value, but the
                // cell can still have a comment attached (Excel happily
                // emits `<c r="B2" s="3"/>` for a style-only cell, and
                // "Insert Comment" on that same cell is perfectly legal).
                // Emit only when an annotation exists so the sheet's
                // authored content surfaces without regressing the
                // current no-comment path.
                if sheet_ctx.comment_for_cell(current_row, col_idx).is_some() {
                    emit_sheet_cell(
                        &sheet_ctx,
                        current_row,
                        col_idx,
                        "",
                        profile.heuristic_header_row,
                        primitives,
                        raw_ctx.source_order,
                    );
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(IdpError::Decode {
                    context: "xlsx raw sheet streaming".to_string(),
                    message: format!("Failed to parse sheet '{}': {}", sheet.name, e),
                });
            }
            _ => {}
        }
        buf.clear();
    }

    // Union-iteration pass: surface comments on cells that never appear
    // in the streaming XML at all. `BTreeMap` key order gives us stable
    // (row-major) output, matching the layout the data-cell pass emits.
    for &(row, col) in sheet.native_metadata.comments.keys() {
        if visited.contains(&(row, col)) {
            continue;
        }
        emit_sheet_cell(
            &sheet_ctx,
            row,
            col,
            "",
            profile.heuristic_header_row,
            primitives,
            raw_ctx.source_order,
        );
    }

    Ok(())
}

fn emit_sheet_cell(
    ctx: &SheetContext,
    row_idx: u32,
    col_idx: u32,
    content: &str,
    heuristic_header_row: Option<u32>,
    primitives: &mut impl PrimitiveSink,
    source_order: &mut u64,
) {
    let comment = ctx.comment_for_cell(row_idx, col_idx);
    // Skip an empty cell only when it has no comment either — a cell that
    // carries only an annotation is still meaningful and must surface.
    if (content.is_empty() && comment.is_none()) || ctx.is_merged_non_origin(row_idx, col_idx) {
        return;
    }
    let content = match comment {
        Some(note) if !note.is_empty() => annotate_with_comment(content, note),
        _ => content.to_string(),
    };
    let content = content.as_str();

    let (raw_rowspan, raw_colspan) = ctx.merge_span(row_idx, col_idx);
    let rowspan = ctx.visible_row_span(row_idx, raw_rowspan);
    let colspan = ctx.visible_col_span(col_idx, raw_colspan);

    let x = ctx.visible_col_position(col_idx) as f32 / ctx.visible_cols as f32;
    let y = ctx.visible_row_position(row_idx) as f32 / ctx.visible_rows as f32;
    let w = colspan as f32 / ctx.visible_cols as f32;
    let h = rowspan as f32 / ctx.visible_rows as f32;

    let mut hints = Vec::new();
    if let Some(table) = ctx.native_table_for_cell(row_idx, col_idx) {
        hints.push(native_table_hint(table, row_idx, col_idx, rowspan, colspan));
    } else if heuristic_header_row == Some(row_idx) {
        hints.push(SemanticHint::from_format(HintKind::TableHeader {
            col: col_idx,
        }));
    } else if ctx.native_tables.is_empty() {
        hints.push(SemanticHint::from_format(HintKind::TableCell {
            row: row_idx,
            col: col_idx,
            rowspan,
            colspan,
        }));
    }

    if let Some(target) = ctx.hyperlink_for_cell(row_idx, col_idx) {
        hints.push(SemanticHint::from_format(HintKind::Link {
            url: target.to_string(),
        }));
    }

    let prim = Primitive::reconstructed(
        PrimitiveKind::Text {
            content: content.nfc().collect(),
            font_size: 11.0,
            font_name: String::new(),
            is_bold: hints
                .iter()
                .any(|h| matches!(h.kind, HintKind::TableHeader { .. })),
            is_italic: false,
            color: None,
            char_positions: None,
            text_direction: TextDirection::default(),
            text_direction_origin: ValueOrigin::Synthetic,
            lang: None,
            lang_origin: None,
        },
        BoundingBox::new(x, y, w, h),
        ctx.page,
        *source_order,
        GeometrySpace::LogicalGrid,
    );
    let prim = if hints.is_empty() {
        prim
    } else {
        prim.with_hints(hints)
    };
    primitives.emit(prim);
    *source_order += 1;
}

/// Emit a top-of-sheet heading carrying the worksheet name.
///
/// Workbooks frequently split semantically distinct tables across sheets
/// (a "Summary" tab, a "Details" tab, a "Notes" tab). Without the sheet
/// name in the output, a consumer only sees concatenated tables and must
/// guess where one ends and the next begins. Pandas (`pd.read_excel`
/// returning a `{sheet_name: DataFrame}` dict) and Apache POI preserve
/// this signal unconditionally. We emit it as a `HintKind::Heading { level: 1 }`
/// so the structure assembler treats it as a top-level heading on the page
/// corresponding to that sheet.
fn emit_sheet_title(
    page: u32,
    sheet_name: &str,
    primitives: &mut impl PrimitiveSink,
    source_order: &mut u64,
) {
    if sheet_name.trim().is_empty() {
        return;
    }
    let prim = Primitive::reconstructed(
        PrimitiveKind::Text {
            content: sheet_name.nfc().collect(),
            font_size: 14.0,
            font_name: String::new(),
            is_bold: true,
            is_italic: false,
            color: None,
            char_positions: None,
            text_direction: TextDirection::default(),
            text_direction_origin: ValueOrigin::Synthetic,
            lang: None,
            lang_origin: None,
        },
        BoundingBox::new(0.0, 0.0, 1.0, 0.0),
        page,
        *source_order,
        GeometrySpace::LogicalGrid,
    )
    .with_hint(SemanticHint::from_format(HintKind::Heading { level: 1 }));
    primitives.emit(prim);
    *source_order += 1;
}

fn warn_if_heuristic_header_row(
    sheet_name: &str,
    page: u32,
    heuristic_header_row: Option<u32>,
    warnings: &mut Vec<Warning>,
) {
    if let Some(header_row) = heuristic_header_row {
        warnings.push(Warning {
            kind: WarningKind::HeuristicInference,
            message: format!(
                "Sheet '{}' has no native SpreadsheetML table metadata; treating row {} as a header via all-string fallback heuristic",
                sheet_name,
                header_row + 1
            ),
            page: Some(page),
        });
    }
}

fn native_table_hint(
    table: &NativeTable,
    row_idx: u32,
    col_idx: u32,
    rowspan: u32,
    colspan: u32,
) -> SemanticHint {
    let rel_row = row_idx.saturating_sub(table.start_row);
    let rel_col = col_idx.saturating_sub(table.start_col);
    if table.is_header_cell(row_idx, col_idx) {
        SemanticHint::from_format(HintKind::TableHeader { col: rel_col })
    } else {
        SemanticHint::from_format(HintKind::TableCell {
            row: rel_row,
            col: rel_col,
            rowspan,
            colspan,
        })
    }
}

/// Attach a cell comment to the cell's displayed text.
///
/// The convention — cell value, then ` [note: …]` — keeps the value first
/// (so downstream extractors and human readers see it at the head of the
/// cell) and the annotation distinct and easy to strip. Multi-line
/// threaded replies collapse to a single-line `[note: …]` with reply
/// arrows because the sheet cell is a single-line rendering target; the
/// full thread with newlines remains in `SheetNativeMetadata::comments`
/// for callers who want the structured form.
fn annotate_with_comment(content: &str, note: &str) -> String {
    let flattened = note.replace('\n', " ");
    if content.is_empty() {
        format!("[note: {}]", flattened)
    } else {
        format!("{} [note: {}]", content, flattened)
    }
}

fn cell_to_string(cell: Option<&Data>) -> String {
    match cell {
        Some(Data::String(s)) => s.clone(),
        Some(Data::Float(f)) => {
            if *f == (*f as i64) as f64 && f.abs() < i64::MAX as f64 {
                format!("{}", *f as i64)
            } else {
                format!("{}", f)
            }
        }
        Some(Data::Int(i)) => format!("{}", i),
        Some(Data::Bool(b)) => format!("{}", b),
        Some(Data::DateTime(dt)) => format!("{}", dt),
        Some(Data::DateTimeIso(s)) => s.clone(),
        Some(Data::DurationIso(s)) => s.clone(),
        Some(Data::Error(e)) => format!("#ERR:{:?}", e),
        Some(Data::Empty) | None => String::new(),
    }
}
