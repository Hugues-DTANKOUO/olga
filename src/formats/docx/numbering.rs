//! DOCX numbering definition parsing (lists).

use std::collections::HashMap;
use std::io::Cursor;

use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::error::{Warning, WarningKind};
use crate::formats::xml_utils::{attr_value, local_name};

use super::rels::read_zip_entry_opt;
use super::types::{DocxPackageIssueCounts, NumberingLevel};

pub(crate) fn parse_numbering(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    warnings: &mut Vec<Warning>,
    package_issue_counts: &mut DocxPackageIssueCounts,
) -> HashMap<(String, u8), NumberingLevel> {
    let mut result: HashMap<(String, u8), NumberingLevel> = HashMap::new();

    let xml = match read_zip_entry_opt(archive, super::PATH_NUMBERING_XML) {
        Some(x) => x,
        None => return result,
    };

    // Parse abstract numbering definitions first.
    let mut abstract_nums: HashMap<String, HashMap<u8, NumberingLevel>> = HashMap::new();
    let mut num_to_abstract: HashMap<String, String> = HashMap::new();

    let mut reader = Reader::from_reader(xml.as_slice());
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    let mut current_abstract_id: Option<String> = None;
    let mut current_level: Option<u8> = None;
    let mut current_num_id: Option<String> = None;
    let mut current_fmt: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let qn = e.name();
                let name = local_name(qn.as_ref());
                match name.as_slice() {
                    b"abstractNum" => {
                        for attr in e.attributes().flatten() {
                            if local_name(attr.key.as_ref()) == b"abstractNumId" {
                                current_abstract_id = Some(attr_value(&attr));
                            }
                        }
                    }
                    b"lvl" => {
                        for attr in e.attributes().flatten() {
                            if local_name(attr.key.as_ref()) == b"ilvl" {
                                current_level = attr_value(&attr).parse().ok();
                            }
                        }
                    }
                    b"num" => {
                        for attr in e.attributes().flatten() {
                            if local_name(attr.key.as_ref()) == b"numId" {
                                current_num_id = Some(attr_value(&attr));
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let qn = e.name();
                let name = local_name(qn.as_ref());
                match name.as_slice() {
                    b"numFmt" => {
                        for attr in e.attributes().flatten() {
                            if local_name(attr.key.as_ref()) == b"val" {
                                current_fmt = Some(attr_value(&attr));
                            }
                        }
                    }
                    b"abstractNumId" if current_num_id.is_some() => {
                        for attr in e.attributes().flatten() {
                            if local_name(attr.key.as_ref()) == b"val"
                                && let Some(ref nid) = current_num_id
                            {
                                num_to_abstract.insert(nid.clone(), attr_value(&attr));
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let qn = e.name();
                let name = local_name(qn.as_ref());
                match name.as_slice() {
                    b"lvl" => {
                        if let (Some(abs_id), Some(level), Some(fmt)) =
                            (&current_abstract_id, current_level, &current_fmt)
                        {
                            let is_ordered = !matches!(fmt.as_str(), "bullet" | "none");
                            abstract_nums
                                .entry(abs_id.clone())
                                .or_default()
                                .insert(level, NumberingLevel { is_ordered });
                        }
                        current_level = None;
                        current_fmt = None;
                    }
                    b"abstractNum" => {
                        current_abstract_id = None;
                    }
                    b"num" => {
                        current_num_id = None;
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                package_issue_counts.malformed_auxiliary_xml_files += 1;
                warnings.push(Warning {
                    kind: WarningKind::MalformedContent,
                    message: format!("XML parse error in numbering.xml: {}", e),
                    page: None,
                });
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    // Resolve: numId → abstractNumId → levels.
    for (num_id, abs_id) in &num_to_abstract {
        if let Some(levels) = abstract_nums.get(abs_id) {
            for (level, def) in levels {
                result.insert((num_id.clone(), *level), def.clone());
            }
        } else {
            package_issue_counts.broken_numbering_defs += 1;
            warnings.push(Warning {
                kind: WarningKind::PartialExtraction,
                message: format!(
                    "Numbering definition '{}' references missing abstract numbering '{}'",
                    num_id, abs_id
                ),
                page: None,
            });
        }
    }

    result
}
