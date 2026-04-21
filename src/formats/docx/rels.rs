//! ZIP and relationship (.rels) parsing for DOCX.

use std::collections::HashMap;
use std::io::{Cursor, Read};

use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::error::{IdpError, IdpResult, Warning, WarningKind};
use crate::formats::xml_utils::{attr_value, local_name};

use super::types::{DocxPackageIssueCounts, RelType, Relationship};

// ---------------------------------------------------------------------------
// ZIP helpers
// ---------------------------------------------------------------------------

pub(crate) fn read_zip_entry(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    path: &str,
) -> IdpResult<Vec<u8>> {
    let mut entry = archive
        .by_name(path)
        .map_err(|_| IdpError::MissingPart(path.to_string()))?;
    let mut buf = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut buf)?;
    Ok(buf)
}

pub(crate) fn read_zip_entry_opt(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    path: &str,
) -> Option<Vec<u8>> {
    read_zip_entry(archive, path).ok()
}

pub(crate) fn estimate_page_count(archive: &mut zip::ZipArchive<Cursor<&[u8]>>) -> Option<u32> {
    read_app_page_count(archive).ok().flatten()
}

pub(crate) fn estimate_page_count_with_warnings(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    warnings: &mut Vec<Warning>,
    package_issue_counts: &mut DocxPackageIssueCounts,
) -> Option<u32> {
    match read_app_page_count(archive) {
        Ok(count) => count,
        Err(message) => {
            package_issue_counts.malformed_auxiliary_xml_files += 1;
            warnings.push(Warning {
                kind: WarningKind::MalformedContent,
                message,
                page: None,
            });
            None
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DocumentPageCounts {
    pub rendered_pages: Option<u32>,
    pub explicit_pages: Option<u32>,
}

pub(crate) fn collect_media(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    rels: &HashMap<String, Relationship>,
    warnings: &mut Vec<Warning>,
    package_issue_counts: &mut DocxPackageIssueCounts,
) -> HashMap<String, Vec<u8>> {
    let mut media = HashMap::new();
    for (rid, rel) in rels {
        if rel.rel_type == RelType::Image && !rel.is_external {
            let path = format!("{}{}", super::PATH_WORD_PREFIX, rel.target);
            match read_zip_entry(archive, &path) {
                Ok(data) => {
                    media.insert(rid.clone(), data);
                }
                Err(_) => {
                    package_issue_counts.missing_media_payloads += 1;
                    warnings.push(Warning {
                        kind: WarningKind::MissingMedia,
                        message: format!("Media not found in ZIP: {} (rel {})", path, rid),
                        page: None,
                    });
                }
            }
        }
    }
    media
}

// ---------------------------------------------------------------------------
// Relationship parsing
// ---------------------------------------------------------------------------

pub(crate) fn parse_rels(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    path: &str,
    warnings: &mut Vec<Warning>,
    warn_if_missing: bool,
    package_issue_counts: &mut DocxPackageIssueCounts,
) -> HashMap<String, Relationship> {
    let mut rels = HashMap::new();
    let xml = match read_zip_entry_opt(archive, path) {
        Some(x) => x,
        None => {
            if warn_if_missing {
                warnings.push(Warning {
                    kind: WarningKind::MissingPart,
                    message: format!("Relationship part missing: {}", path),
                    page: None,
                });
            }
            return rels;
        }
    };

    let mut reader = Reader::from_reader(xml.as_slice());
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e))
                if local_name(e.name().as_ref()) == b"Relationship".as_slice() =>
            {
                let mut id = String::new();
                let mut rel_type = String::new();
                let mut target = String::new();
                let mut is_external = false;

                for attr in e.attributes().flatten() {
                    match local_name(attr.key.as_ref()).as_slice() {
                        b"Id" => id = attr_value(&attr),
                        b"Type" => rel_type = attr_value(&attr),
                        b"Target" => target = attr_value(&attr),
                        b"TargetMode" => {
                            is_external = attr_value(&attr).eq_ignore_ascii_case("External")
                        }
                        _ => {}
                    }
                }

                if !id.is_empty() {
                    rels.insert(
                        id,
                        Relationship {
                            rel_type: RelType::from_uri(&rel_type),
                            target,
                            is_external,
                        },
                    );
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                package_issue_counts.malformed_relationship_parts += 1;
                warnings.push(Warning {
                    kind: WarningKind::MalformedContent,
                    message: format!("XML parse error in {}: {}", path, e),
                    page: None,
                });
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    rels
}

fn read_app_page_count(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
) -> Result<Option<u32>, String> {
    let xml = match read_zip_entry_opt(archive, "docProps/app.xml") {
        Some(xml) => xml,
        None => return Ok(None),
    };

    parse_app_page_count(&xml).map_err(|e| format!("Failed to parse docProps/app.xml: {}", e))
}

fn parse_app_page_count(xml: &[u8]) -> Result<Option<u32>, quick_xml::Error> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut in_pages = false;

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(ref e) => {
                in_pages = local_name(e.name().as_ref()) == b"Pages";
            }
            Event::End(ref e) if local_name(e.name().as_ref()) == b"Pages" => {
                in_pages = false;
            }
            Event::Text(e) if in_pages => {
                let raw = String::from_utf8_lossy(e.as_ref()).into_owned();
                let count = raw.trim().parse::<u32>().ok().filter(|count| *count > 0);
                return Ok(count);
            }
            Event::Eof => return Ok(None),
            _ => {}
        }
        buf.clear();
    }
}

pub(crate) fn parse_document_page_counts(
    xml: &[u8],
) -> Result<DocumentPageCounts, quick_xml::Error> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut rendered_breaks = 0_u32;
    let mut explicit_breaks = 0_u32;

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Empty(ref e) | Event::Start(ref e) => {
                let name = local_name(e.name().as_ref());
                if name == b"lastRenderedPageBreak" {
                    rendered_breaks += 1;
                } else if name == b"br" {
                    let is_page_break = e.attributes().flatten().any(|attr| {
                        local_name(attr.key.as_ref()) == b"type" && attr_value(&attr) == "page"
                    });
                    if is_page_break {
                        explicit_breaks += 1;
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(DocumentPageCounts {
        rendered_pages: (rendered_breaks > 0).then_some(rendered_breaks + 1),
        explicit_pages: (explicit_breaks > 0).then_some(explicit_breaks + 1),
    })
}
