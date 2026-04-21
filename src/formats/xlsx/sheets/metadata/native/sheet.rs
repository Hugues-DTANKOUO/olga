use std::collections::HashMap;
use std::io::Cursor;

use quick_xml::events::Event;
use quick_xml::reader::Reader as XmlReader;

use crate::error::{Warning, WarningKind};
use crate::formats::xml_utils::{attr_value, local_name};

use crate::formats::xlsx::sheets::PendingSheetHyperlink;
use crate::formats::xlsx::types::{SheetHyperlink, SheetNativeMetadata};

use super::super::cells::{parse_cell_range, parse_cell_rect_ref};
use super::super::comments::enrich_sheet_comments;
use super::super::relationships::{
    parse_relationship_targets, read_zip_entry_optional, read_zip_entry_raw,
    relationships_path_for_part, resolve_relationship_target,
};
use super::table::parse_table_xml;

pub(in super::super::super) fn parse_sheet_native_metadata(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    sheet_name: &str,
    sheet_path: &str,
    xml: &[u8],
    page: u32,
    warnings: &mut Vec<Warning>,
) -> SheetNativeMetadata {
    let mut metadata = SheetNativeMetadata::default();
    let mut table_relationship_ids = Vec::new();
    let mut pending_hyperlinks = Vec::new();
    let mut reader = XmlReader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                match local_name(e.name().as_ref()).as_slice() {
                    b"mergeCell" => {
                        for attr in e.attributes().flatten() {
                            if local_name(attr.key.as_ref()) == b"ref" {
                                let value = attr_value(&attr);
                                if let Some(range) = parse_cell_range(&value) {
                                    metadata.merged.push(range);
                                }
                            }
                        }
                    }
                    b"tablePart" => {
                        for attr in e.attributes().flatten() {
                            if local_name(attr.key.as_ref()) == b"id" {
                                table_relationship_ids.push(attr_value(&attr));
                            }
                        }
                    }
                    b"hyperlink" => {
                        let mut reference = None;
                        let mut relationship_id = None;
                        let mut location = None;
                        for attr in e.attributes().flatten() {
                            match local_name(attr.key.as_ref()).as_slice() {
                                b"ref" => reference = Some(attr_value(&attr)),
                                b"id" => relationship_id = Some(attr_value(&attr)),
                                b"location" => location = Some(attr_value(&attr)),
                                _ => {}
                            }
                        }

                        let Some(reference) = reference else {
                            warnings.push(Warning {
                                kind: WarningKind::MalformedContent,
                                message: format!(
                                    "Hyperlink metadata is missing a ref attribute in sheet '{}'",
                                    sheet_name
                                ),
                                page: Some(page),
                            });
                            continue;
                        };

                        let Some((start_row, start_col, end_row, end_col)) =
                            parse_cell_rect_ref(&reference)
                        else {
                            warnings.push(Warning {
                                kind: WarningKind::MalformedContent,
                                message: format!(
                                    "Hyperlink ref '{}' in sheet '{}' is malformed",
                                    reference, sheet_name
                                ),
                                page: Some(page),
                            });
                            continue;
                        };

                        pending_hyperlinks.push(PendingSheetHyperlink {
                            start_row,
                            start_col,
                            end_row,
                            end_col,
                            relationship_id,
                            location,
                        });
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                warnings.push(Warning {
                    kind: WarningKind::MalformedContent,
                    message: format!(
                        "Failed to parse native sheet metadata for sheet '{}': {}",
                        sheet_name, e
                    ),
                    page: Some(page),
                });
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    metadata.merged.sort_by_key(|range| {
        (
            range.start_row,
            range.start_col,
            range.rowspan,
            range.colspan,
        )
    });

    let rels_path = relationships_path_for_part(sheet_path);
    // Per OPC (ECMA-376-2 § 10) a worksheet's `_rels/<sheet>.xml.rels`
    // file is only present when the sheet has at least one relationship.
    // Worksheets without comments, tables, drawings, hyperlinks, or
    // external links legitimately have no rels part — its absence must
    // not produce a MissingPart warning. We always probe (even when
    // inline tables/hyperlinks didn't appear) because comments and
    // threaded comments are discovered only via rels.
    let rels_xml = read_zip_entry_optional(archive, &rels_path, Some(page), warnings);

    if let Some(rels_xml) = rels_xml.as_deref() {
        enrich_sheet_comments(
            archive,
            sheet_name,
            sheet_path,
            rels_xml,
            &mut metadata,
            page,
            warnings,
        );
    }

    if table_relationship_ids.is_empty() && pending_hyperlinks.is_empty() {
        return metadata;
    }

    let rels_xml = match rels_xml {
        Some(xml) => xml,
        None => return metadata,
    };

    let rel_targets = match parse_relationship_targets(&rels_xml, Some("table")) {
        Ok(targets) => targets,
        Err(e) => {
            warnings.push(Warning {
                kind: WarningKind::MalformedContent,
                message: format!(
                    "Failed to parse worksheet relationships '{}': {}",
                    rels_path, e
                ),
                page: Some(page),
            });
            return metadata;
        }
    };
    let hyperlink_targets = match parse_relationship_targets(&rels_xml, Some("hyperlink")) {
        Ok(targets) => targets,
        Err(e) => {
            warnings.push(Warning {
                kind: WarningKind::MalformedContent,
                message: format!(
                    "Failed to parse worksheet hyperlink relationships '{}': {}",
                    rels_path, e
                ),
                page: Some(page),
            });
            HashMap::new()
        }
    };

    for relationship_id in table_relationship_ids {
        let Some(target) = rel_targets.get(&relationship_id) else {
            warnings.push(Warning {
                kind: WarningKind::UnresolvedRelationship,
                message: format!(
                    "Table relationship '{}' could not be resolved for sheet '{}'",
                    relationship_id, sheet_name
                ),
                page: Some(page),
            });
            continue;
        };

        let table_path = resolve_relationship_target(sheet_path, target);
        let table_xml = match read_zip_entry_raw(archive, &table_path, Some(page), warnings) {
            Some(xml) => xml,
            None => continue,
        };

        if let Some(table) = parse_table_xml(&table_xml, sheet_name, page, warnings) {
            metadata.native_tables.push(table);
        }
    }

    for hyperlink in pending_hyperlinks {
        let target = match (
            hyperlink.relationship_id.as_deref(),
            hyperlink.location.as_deref(),
        ) {
            (Some(rel_id), Some(location)) => hyperlink_targets
                .get(rel_id)
                .map(|base| format!("{}#{}", base, location))
                .or_else(|| Some(format!("#{location}"))),
            (Some(rel_id), None) => {
                let Some(target) = hyperlink_targets.get(rel_id) else {
                    warnings.push(Warning {
                        kind: WarningKind::UnresolvedRelationship,
                        message: format!(
                            "Hyperlink relationship '{}' could not be resolved for sheet '{}'",
                            rel_id, sheet_name
                        ),
                        page: Some(page),
                    });
                    continue;
                };
                Some(target.clone())
            }
            (None, Some(location)) => Some(format!("#{location}")),
            (None, None) => {
                warnings.push(Warning {
                    kind: WarningKind::MalformedContent,
                    message: format!(
                        "Hyperlink metadata in sheet '{}' did not expose either r:id or location",
                        sheet_name
                    ),
                    page: Some(page),
                });
                None
            }
        };

        if let Some(target) = target {
            metadata.hyperlinks.push(SheetHyperlink {
                start_row: hyperlink.start_row,
                start_col: hyperlink.start_col,
                end_row: hyperlink.end_row,
                end_col: hyperlink.end_col,
                target,
            });
        }
    }

    metadata.native_tables.sort_by_key(|table| {
        (
            table.start_row,
            table.start_col,
            table.end_row,
            table.end_col,
        )
    });
    metadata.hyperlinks.sort_by_key(|link| {
        (
            link.start_row,
            link.start_col,
            link.end_row,
            link.end_col,
            link.target.clone(),
        )
    });
    metadata
}
