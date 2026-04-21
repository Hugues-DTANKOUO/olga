use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};

use quick_xml::events::Event;
use quick_xml::reader::Reader as XmlReader;

use crate::error::{Warning, WarningKind};
use crate::formats::xml_utils::{attr_value, local_name};

use super::super::SheetDescriptor;

pub(in super::super) fn parse_workbook_sheet_descriptors(
    xml: &[u8],
) -> Result<Vec<SheetDescriptor>, quick_xml::Error> {
    let mut sheets = Vec::new();
    let mut reader = XmlReader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Empty(ref e) | Event::Start(ref e)
                if local_name(e.name().as_ref()) == b"sheet" =>
            {
                let mut name = None;
                let mut relationship_id = None;
                for attr in e.attributes().flatten() {
                    match local_name(attr.key.as_ref()).as_slice() {
                        b"name" => name = Some(attr_value(&attr)),
                        b"id" => relationship_id = Some(attr_value(&attr)),
                        _ => {}
                    }
                }

                if let (Some(name), Some(relationship_id)) = (name, relationship_id) {
                    sheets.push(SheetDescriptor {
                        name,
                        relationship_id,
                    });
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(sheets)
}

pub(in super::super) fn parse_relationship_targets(
    xml: &[u8],
    expected_type_suffix: Option<&str>,
) -> Result<HashMap<String, String>, quick_xml::Error> {
    let mut targets = HashMap::new();
    let mut reader = XmlReader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Empty(ref e) | Event::Start(ref e)
                if local_name(e.name().as_ref()) == b"Relationship" =>
            {
                let mut id = None;
                let mut rel_type = None;
                let mut target = None;
                for attr in e.attributes().flatten() {
                    match local_name(attr.key.as_ref()).as_slice() {
                        b"Id" => id = Some(attr_value(&attr)),
                        b"Type" => rel_type = Some(attr_value(&attr)),
                        b"Target" => target = Some(attr_value(&attr)),
                        _ => {}
                    }
                }

                let matches_type = expected_type_suffix.is_none_or(|suffix| {
                    rel_type
                        .as_deref()
                        .is_some_and(|rel_type| rel_type.ends_with(suffix))
                });

                if matches_type && let (Some(id), Some(target)) = (id, target) {
                    targets.insert(id, target);
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(targets)
}

pub(in super::super) fn read_zip_entry_raw(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    path: &str,
    page: Option<u32>,
    warnings: &mut Vec<Warning>,
) -> Option<Vec<u8>> {
    let mut entry = match archive.by_name(path) {
        Ok(entry) => entry,
        Err(_) => {
            warnings.push(Warning {
                kind: WarningKind::MissingPart,
                message: format!("Worksheet XML part missing: {}", path),
                page,
            });
            return None;
        }
    };
    let mut buf = Vec::with_capacity(entry.size() as usize);
    if let Err(e) = entry.read_to_end(&mut buf) {
        warnings.push(Warning {
            kind: WarningKind::PartialExtraction,
            message: format!("Failed to read ZIP part '{}': {}", path, e),
            page,
        });
        return None;
    }
    Some(buf)
}

/// Probe for an OPC part that is allowed to be absent by the spec.
///
/// In OPC (ECMA-376-2 § 10) a part's `_rels/<part>.rels` file is only
/// present when the part has at least one relationship. Absence is NOT
/// a document defect — it's the normal case for a worksheet with no
/// external links, comments, drawings, or tables. Emitting a
/// [`WarningKind::MissingPart`] for that absence would pollute the
/// output with false positives, so optional reads swallow the not-found
/// case silently while still surfacing real I/O failures.
pub(in super::super) fn read_zip_entry_optional(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    path: &str,
    page: Option<u32>,
    warnings: &mut Vec<Warning>,
) -> Option<Vec<u8>> {
    let mut entry = match archive.by_name(path) {
        Ok(entry) => entry,
        Err(_) => return None,
    };
    let mut buf = Vec::with_capacity(entry.size() as usize);
    if let Err(e) = entry.read_to_end(&mut buf) {
        warnings.push(Warning {
            kind: WarningKind::PartialExtraction,
            message: format!("Failed to read ZIP part '{}': {}", path, e),
            page,
        });
        return None;
    }
    Some(buf)
}

pub(in super::super) fn resolve_relationship_target(base_path: &str, target: &str) -> String {
    if target.starts_with('/') {
        return target.trim_start_matches('/').to_string();
    }

    let joined = Path::new(base_path)
        .parent()
        .map(|parent| parent.join(target))
        .unwrap_or_else(|| PathBuf::from(target));

    let mut normalized = Vec::new();
    for component in joined.components() {
        match component {
            Component::Normal(part) => normalized.push(part.to_string_lossy().to_string()),
            Component::ParentDir => {
                let _ = normalized.pop();
            }
            Component::CurDir | Component::RootDir | Component::Prefix(_) => {}
        }
    }

    normalized.join("/")
}

pub(in super::super) fn relationships_path_for_part(part_path: &str) -> String {
    let path = Path::new(part_path);
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    if parent.as_os_str().is_empty() {
        format!("_rels/{}.rels", file_name)
    } else {
        format!("{}/_rels/{}.rels", parent.to_string_lossy(), file_name)
    }
}
