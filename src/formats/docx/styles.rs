//! DOCX style parsing and inheritance resolution.

use std::collections::HashMap;
use std::io::Cursor;

use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::error::{Warning, WarningKind};
use crate::formats::xml_utils::{attr_value, bool_attr, local_name};

use super::rels::read_zip_entry_opt;
use super::types::{DocxPackageIssueCounts, ResolvedStyle};

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub(crate) fn parse_styles(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    warnings: &mut Vec<Warning>,
    package_issue_counts: &mut DocxPackageIssueCounts,
) -> HashMap<String, ResolvedStyle> {
    let mut styles: HashMap<String, ResolvedStyle> = HashMap::new();

    let xml = match read_zip_entry_opt(archive, super::PATH_STYLES_XML) {
        Some(x) => x,
        None => {
            warnings.push(Warning {
                kind: WarningKind::MissingPart,
                message: format!(
                    "Optional style part missing: {}. Paragraphs will use decoder defaults.",
                    super::PATH_STYLES_XML
                ),
                page: None,
            });
            return styles;
        }
    };

    // First pass: parse raw styles.
    let raw_styles = parse_raw_styles(&xml, warnings, package_issue_counts);

    // Second pass: resolve inheritance.
    let style_ids: Vec<String> = raw_styles.keys().cloned().collect();
    for id in &style_ids {
        let resolved = resolve_style_internal(id, &raw_styles, 0, warnings, package_issue_counts);
        styles.insert(id.clone(), resolved);
    }

    styles
}

// ---------------------------------------------------------------------------
// Raw style parsing
// ---------------------------------------------------------------------------

fn parse_raw_styles(
    xml: &[u8],
    warnings: &mut Vec<Warning>,
    package_issue_counts: &mut DocxPackageIssueCounts,
) -> HashMap<String, ResolvedStyle> {
    let mut styles = HashMap::new();
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    let mut current_style: Option<ResolvedStyle> = None;
    let mut in_rpr = false;
    let mut in_ppr = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let qn = e.name();
                let name = local_name(qn.as_ref());
                match name.as_slice() {
                    b"style" => {
                        let mut style = ResolvedStyle::default();
                        for attr in e.attributes().flatten() {
                            if local_name(attr.key.as_ref()).as_slice() == b"styleId" {
                                style.style_id = attr_value(&attr);
                            }
                        }
                        current_style = Some(style);
                    }
                    b"rPr" if current_style.is_some() => in_rpr = true,
                    b"pPr" if current_style.is_some() => in_ppr = true,
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let qn = e.name();
                let name = local_name(qn.as_ref());
                if let Some(ref mut style) = current_style {
                    match name.as_slice() {
                        b"name" => {
                            for attr in e.attributes().flatten() {
                                if local_name(attr.key.as_ref()) == b"val".as_slice() {
                                    style.name = attr_value(&attr);
                                }
                            }
                        }
                        b"basedOn" => {
                            for attr in e.attributes().flatten() {
                                if local_name(attr.key.as_ref()) == b"val" {
                                    style.based_on = Some(attr_value(&attr));
                                }
                            }
                        }
                        b"b" if in_rpr => {
                            style.is_bold = Some(bool_attr(e, true));
                        }
                        b"i" if in_rpr => {
                            style.is_italic = Some(bool_attr(e, true));
                        }
                        b"sz" if in_rpr => {
                            for attr in e.attributes().flatten() {
                                if local_name(attr.key.as_ref()) == b"val"
                                    && let Ok(v) = attr_value(&attr).parse::<u32>()
                                {
                                    style.font_size_half_pt = Some(v);
                                }
                            }
                        }
                        b"rFonts" if in_rpr => {
                            for attr in e.attributes().flatten() {
                                let k = local_name(attr.key.as_ref());
                                if k == b"ascii" || k == b"hAnsi" || k == b"cs" {
                                    style.font_name = Some(attr_value(&attr));
                                    break;
                                }
                            }
                        }
                        b"color" if in_rpr => {
                            for attr in e.attributes().flatten() {
                                if local_name(attr.key.as_ref()) == b"val" {
                                    style.color = Some(attr_value(&attr));
                                }
                            }
                        }
                        b"outlineLvl" if in_ppr => {
                            for attr in e.attributes().flatten() {
                                if local_name(attr.key.as_ref()) == b"val"
                                    && let Ok(v) = attr_value(&attr).parse::<u8>()
                                {
                                    style.outline_level = Some(v);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let qn = e.name();
                let name = local_name(qn.as_ref());
                match name.as_slice() {
                    b"style" => {
                        if let Some(style) = current_style.take()
                            && !style.style_id.is_empty()
                        {
                            styles.insert(style.style_id.clone(), style);
                        }
                    }
                    b"rPr" => in_rpr = false,
                    b"pPr" => in_ppr = false,
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                package_issue_counts.malformed_auxiliary_xml_files += 1;
                warnings.push(Warning {
                    kind: WarningKind::MalformedContent,
                    message: format!("XML parse error in styles.xml: {}", e),
                    page: None,
                });
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    styles
}

// ---------------------------------------------------------------------------
// Inheritance resolution
// ---------------------------------------------------------------------------

/// Maximum style inheritance depth to prevent infinite loops on circular references.
const MAX_STYLE_DEPTH: usize = 10;

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn resolve_style(
    id: &str,
    raw: &HashMap<String, ResolvedStyle>,
    depth: usize,
) -> ResolvedStyle {
    let mut warnings = Vec::new();
    let mut package_issue_counts = DocxPackageIssueCounts::default();
    resolve_style_internal(id, raw, depth, &mut warnings, &mut package_issue_counts)
}

fn resolve_style_internal(
    id: &str,
    raw: &HashMap<String, ResolvedStyle>,
    depth: usize,
    warnings: &mut Vec<Warning>,
    package_issue_counts: &mut DocxPackageIssueCounts,
) -> ResolvedStyle {
    if depth > MAX_STYLE_DEPTH {
        package_issue_counts.circular_style_chains += 1;
        warnings.push(Warning {
            kind: WarningKind::UnresolvedStyle,
            message: format!(
                "Style inheritance depth exceeded ({}) resolving '{}' — possible circular reference",
                MAX_STYLE_DEPTH, id
            ),
            page: None,
        });
        return ResolvedStyle::default();
    }

    let style = match raw.get(id) {
        Some(s) => s.clone(),
        None => {
            package_issue_counts.missing_style_definitions += 1;
            warnings.push(Warning {
                kind: WarningKind::UnresolvedStyle,
                message: format!("Style '{}' referenced but not defined in styles.xml", id),
                page: None,
            });
            return ResolvedStyle::default();
        }
    };

    if let Some(ref parent_id) = style.based_on {
        let parent =
            resolve_style_internal(parent_id, raw, depth + 1, warnings, package_issue_counts);
        ResolvedStyle {
            name: style.name.clone(),
            style_id: style.style_id.clone(),
            font_size_half_pt: style.font_size_half_pt.or(parent.font_size_half_pt),
            font_name: style.font_name.clone().or(parent.font_name),
            is_bold: style.is_bold.or(parent.is_bold),
            is_italic: style.is_italic.or(parent.is_italic),
            color: style.color.clone().or(parent.color),
            outline_level: style.outline_level.or(parent.outline_level),
            based_on: style.based_on.clone(),
        }
    } else {
        style
    }
}
