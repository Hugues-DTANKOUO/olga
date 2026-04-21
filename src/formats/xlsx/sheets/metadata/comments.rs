//! XLSX cell-comment extraction.
//!
//! OOXML carries two coexisting comment systems since Excel 365 introduced
//! threaded conversations in 2018 (MS-XLSX § 2.4.86):
//!
//! * **Legacy notes** — single-author, single-text-block annotations.
//!   Stored in `xl/comments{N}.xml` under root element `<comments>`.
//!   Authors are listed inline as `<authors><author>…</author></authors>`.
//!   The relationship type from a worksheet is
//!   `…/officeDocument/2006/relationships/comments`.
//!
//! * **Threaded comments** — modern conversation threads with reply
//!   chains, timestamps, mentions, and persisted person identities.
//!   Stored in `xl/threadedComments/threadedComment{N}.xml` under root
//!   element `<ThreadedComments>` ([MS-XLSX § 2.3.7][threaded]). The
//!   relationship type from a worksheet is
//!   `http://schemas.microsoft.com/office/2017/10/relationships/threadedComment`.
//!   Author identities live in a workbook-level `xl/persons/person{N}.xml`
//!   keyed by GUID-style `personId`.
//!
//! Modern Excel files written by Office 365 ship **both** simultaneously,
//! the legacy notes acting as a downgrade fallback for older Excel
//! versions that don't recognise threaded comments. When both are present
//! for the same cell we prefer the threaded version because it is the
//! canonical source — older Excel writes a stub like
//! `[Threaded comment] Your version of Excel allows you to read this
//! threaded comment …` into the legacy notes.
//!
//! [threaded]: https://learn.microsoft.com/en-us/openspecs/office_standards/ms-xlsx/66e1875d-c60a-48eb-bf88-41066d45fea8

use std::collections::{BTreeMap, HashMap};
use std::io::Cursor;

use quick_xml::events::Event;
use quick_xml::reader::Reader as XmlReader;

use crate::error::{Warning, WarningKind};
use crate::formats::xml_utils::{attr_value, local_name, resolve_predefined_entity};

use super::super::super::types::SheetNativeMetadata;
use super::cells::parse_cell_ref;
use super::relationships::{
    parse_relationship_targets, read_zip_entry_raw, relationships_path_for_part,
    resolve_relationship_target,
};

/// Suffixes used to match relationship types in worksheet `.rels`.
///
/// We match by suffix (last `/`-separated path component) to be tolerant
/// of namespace differences (the threaded relationship is in the
/// `schemas.microsoft.com` namespace, legacy in `openxmlformats.org`).
const LEGACY_COMMENTS_REL: &str = "comments";
const THREADED_COMMENTS_REL: &str = "threadedComment";

/// Parsed legacy or threaded comment, normalized to the same shape.
#[derive(Debug, Clone)]
struct ParsedComment {
    row: u32,
    col: u32,
    author: Option<String>,
    text: String,
    /// Modern threaded comments include a parent id chain — when set, this
    /// comment is a reply to another. Stored so that ordering / threading
    /// remains stable across runs.
    parent_id: Option<String>,
}

/// Walk the worksheet's relationships graph and merge every comment for
/// the sheet into `metadata.comments`.
///
/// Threaded comments take priority over legacy notes for the same cell,
/// because Office 365 writes a downgrade-stub into legacy when a threaded
/// thread exists for the cell (so the legacy version is essentially noise
/// from a modern reader's perspective).
pub(in super::super) fn enrich_sheet_comments(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    sheet_name: &str,
    sheet_path: &str,
    rels_xml: &[u8],
    metadata: &mut SheetNativeMetadata,
    page: u32,
    warnings: &mut Vec<Warning>,
) {
    let mut rendered: BTreeMap<(u32, u32), String> = BTreeMap::new();

    let legacy_targets = match parse_relationship_targets(rels_xml, Some(LEGACY_COMMENTS_REL)) {
        Ok(targets) => targets,
        Err(e) => {
            warnings.push(Warning {
                kind: WarningKind::MalformedContent,
                message: format!(
                    "Failed to scan worksheet relationships for legacy comments in sheet '{}': {}",
                    sheet_name, e
                ),
                page: Some(page),
            });
            HashMap::new()
        }
    };

    for target in legacy_targets.values() {
        let path = resolve_relationship_target(sheet_path, target);
        let Some(xml) = read_zip_entry_raw(archive, &path, Some(page), warnings) else {
            continue;
        };
        match parse_legacy_comments(&xml) {
            Ok(comments) => {
                for c in comments {
                    if is_threaded_downgrade_stub(&c.text) {
                        continue;
                    }
                    rendered
                        .entry((c.row, c.col))
                        .or_insert_with(|| render_comment(&c));
                }
            }
            Err(e) => warnings.push(Warning {
                kind: WarningKind::MalformedContent,
                message: format!(
                    "Failed to parse legacy comments at '{}' for sheet '{}': {}",
                    path, sheet_name, e
                ),
                page: Some(page),
            }),
        }
    }

    let threaded_targets = match parse_relationship_targets(rels_xml, Some(THREADED_COMMENTS_REL)) {
        Ok(targets) => targets,
        Err(e) => {
            warnings.push(Warning {
                    kind: WarningKind::MalformedContent,
                    message: format!(
                        "Failed to scan worksheet relationships for threaded comments in sheet '{}': {}",
                        sheet_name, e
                    ),
                    page: Some(page),
                });
            HashMap::new()
        }
    };

    if !threaded_targets.is_empty() {
        let persons = load_workbook_persons(archive, warnings);
        for target in threaded_targets.values() {
            let path = resolve_relationship_target(sheet_path, target);
            let Some(xml) = read_zip_entry_raw(archive, &path, Some(page), warnings) else {
                continue;
            };
            match parse_threaded_comments(&xml, &persons) {
                Ok(comments) => {
                    let threads = group_into_threads(comments);
                    for ((row, col), rendered_text) in threads {
                        // Threaded ALWAYS wins.
                        rendered.insert((row, col), rendered_text);
                    }
                }
                Err(e) => warnings.push(Warning {
                    kind: WarningKind::MalformedContent,
                    message: format!(
                        "Failed to parse threaded comments at '{}' for sheet '{}': {}",
                        path, sheet_name, e
                    ),
                    page: Some(page),
                }),
            }
        }
    }

    metadata.comments = rendered;
}

/// Render a single legacy or threaded comment as `Author: text`.
///
/// Author is omitted when unknown; the colon is dropped so unknown-author
/// notes render as just their text. Excel writes the literal string
/// "Unknown Author" (and locale-specific equivalents) for anonymous legacy
/// notes — strip those so they don't pollute the rendered output.
fn render_comment(c: &ParsedComment) -> String {
    match c.author.as_deref() {
        Some(author) if !author.is_empty() && !is_placeholder_author(author) => {
            format!("{}: {}", author, c.text)
        }
        _ => c.text.clone(),
    }
}

/// Detect Excel's locale-neutral placeholder author names. These are
/// emitted by authoring tools when the user didn't stamp a real author on
/// a legacy comment — surfacing them as `Unknown Author: …` would be
/// pure noise.
fn is_placeholder_author(author: &str) -> bool {
    matches!(
        author.trim(),
        "Unknown Author"
            | "unknown author"
            | "Author"
            | "Excel"
            | "Windows User"
            | "Microsoft Office User"
    )
}

/// Group a flat threaded-comments list into per-cell strings, preserving
/// reply order.
///
/// Excel writes threaded comments in oldest-first order within a thread
/// and uses `parentId` only on replies (root comments have no parent).
/// We render each thread as `Author1: …\n  ↳ Author2: …` so consumers see
/// who replied to whom without losing the chronology.
fn group_into_threads(comments: Vec<ParsedComment>) -> Vec<((u32, u32), String)> {
    let mut by_cell: BTreeMap<(u32, u32), Vec<ParsedComment>> = BTreeMap::new();
    for c in comments {
        by_cell.entry((c.row, c.col)).or_default().push(c);
    }

    let mut out = Vec::with_capacity(by_cell.len());
    for ((row, col), thread) in by_cell {
        let mut lines = Vec::with_capacity(thread.len());
        for (idx, comment) in thread.iter().enumerate() {
            let prefix = if idx == 0 || comment.parent_id.is_none() {
                String::new()
            } else {
                "  ↳ ".to_string()
            };
            lines.push(format!("{}{}", prefix, render_comment(comment)));
        }
        out.push(((row, col), lines.join("\n")));
    }
    out
}

/// Office 365 writes a downgrade stub into the legacy `comments.xml` part
/// whenever a threaded comment exists, so older Excel readers see *some*
/// indication a thread is present. We strip those — they carry no
/// information beyond "a thread exists here", and we already pick up the
/// real thread from the threadedComments part.
fn is_threaded_downgrade_stub(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("threaded comment")
        && (lower.contains("your version of excel")
            || lower.contains("doesn't allow you to display")
            || lower.contains("can't be edited"))
}

// ---------------------------------------------------------------------------
// Legacy notes — xl/comments{N}.xml
// ---------------------------------------------------------------------------

fn parse_legacy_comments(xml: &[u8]) -> Result<Vec<ParsedComment>, quick_xml::Error> {
    let mut reader = XmlReader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut authors: Vec<String> = Vec::new();
    let mut comments: Vec<ParsedComment> = Vec::new();

    let mut in_authors = false;
    let mut in_author = false;
    let mut author_buf = String::new();

    let mut current: Option<PendingLegacy> = None;
    let mut in_text = false;
    let mut in_t = false;
    let mut t_buf = String::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let name = local_name(e.name().as_ref());
                match name.as_slice() {
                    b"authors" => in_authors = true,
                    b"author" if in_authors => {
                        in_author = true;
                        author_buf.clear();
                    }
                    b"comment" => {
                        let mut reference = None;
                        let mut author_id = None;
                        for attr in e.attributes().flatten() {
                            match local_name(attr.key.as_ref()).as_slice() {
                                b"ref" => reference = Some(attr_value(&attr)),
                                b"authorId" => {
                                    author_id = attr_value(&attr).parse::<usize>().ok();
                                }
                                _ => {}
                            }
                        }
                        if let Some(r) = reference.as_deref()
                            && let Some((row, col)) = parse_cell_ref(r)
                        {
                            current = Some(PendingLegacy {
                                row,
                                col,
                                author_id,
                                text: String::new(),
                            });
                        }
                    }
                    b"text" if current.is_some() => in_text = true,
                    b"t" if in_text => {
                        in_t = true;
                        t_buf.clear();
                    }
                    _ => {}
                }
            }
            Event::Text(ref e) => {
                if in_author {
                    author_buf.push_str(&String::from_utf8_lossy(e.as_ref()));
                } else if in_t {
                    t_buf.push_str(&String::from_utf8_lossy(e.as_ref()));
                }
            }
            Event::GeneralRef(ref e) if in_author || in_t => {
                let ch = if let Ok(Some(c)) = e.resolve_char_ref() {
                    Some(c)
                } else {
                    let name = String::from_utf8_lossy(e.as_ref());
                    resolve_predefined_entity(&name)
                };
                if let Some(c) = ch {
                    let mut buf_char = [0u8; 4];
                    let s = c.encode_utf8(&mut buf_char);
                    if in_author {
                        author_buf.push_str(s);
                    } else if in_t {
                        t_buf.push_str(s);
                    }
                }
            }
            Event::End(ref e) => {
                let name = local_name(e.name().as_ref());
                match name.as_slice() {
                    b"authors" => in_authors = false,
                    b"author" if in_author => {
                        authors.push(author_buf.trim().to_string());
                        in_author = false;
                    }
                    b"t" if in_t => {
                        if !t_buf.is_empty()
                            && let Some(c) = current.as_mut()
                        {
                            if !c.text.is_empty() {
                                c.text.push(' ');
                            }
                            c.text.push_str(t_buf.trim_end_matches('\n'));
                        }
                        in_t = false;
                    }
                    b"text" => in_text = false,
                    b"comment" => {
                        if let Some(c) = current.take() {
                            let author = c
                                .author_id
                                .and_then(|idx| authors.get(idx).cloned())
                                .filter(|s| !s.is_empty());
                            let text = c.text.trim().to_string();
                            if !text.is_empty() {
                                comments.push(ParsedComment {
                                    row: c.row,
                                    col: c.col,
                                    author,
                                    text,
                                    parent_id: None,
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(comments)
}

struct PendingLegacy {
    row: u32,
    col: u32,
    author_id: Option<usize>,
    text: String,
}

// ---------------------------------------------------------------------------
// Threaded comments — xl/threadedComments/threadedComment{N}.xml
// ---------------------------------------------------------------------------

fn parse_threaded_comments(
    xml: &[u8],
    persons: &HashMap<String, String>,
) -> Result<Vec<ParsedComment>, quick_xml::Error> {
    let mut reader = XmlReader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut comments: Vec<ParsedComment> = Vec::new();

    let mut current: Option<PendingThreaded> = None;
    let mut in_text = false;
    let mut text_buf = String::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(ref e) | Event::Empty(ref e)
                if local_name(e.name().as_ref()) == b"threadedComment" =>
            {
                let mut reference = None;
                let mut person_id = None;
                let mut parent_id = None;
                for attr in e.attributes().flatten() {
                    match local_name(attr.key.as_ref()).as_slice() {
                        b"ref" => reference = Some(attr_value(&attr)),
                        b"personId" => person_id = Some(attr_value(&attr)),
                        b"parentId" => parent_id = Some(attr_value(&attr)),
                        _ => {}
                    }
                }
                if let Some(r) = reference.as_deref()
                    && let Some((row, col)) = parse_cell_ref(r)
                {
                    current = Some(PendingThreaded {
                        row,
                        col,
                        person_id,
                        parent_id,
                        text: String::new(),
                    });
                }
            }
            Event::Start(ref e) if local_name(e.name().as_ref()) == b"text" => {
                in_text = true;
                text_buf.clear();
            }
            Event::Text(ref e) if in_text => {
                text_buf.push_str(&String::from_utf8_lossy(e.as_ref()));
            }
            Event::GeneralRef(ref e) if in_text => {
                let ch = if let Ok(Some(c)) = e.resolve_char_ref() {
                    Some(c)
                } else {
                    let name = String::from_utf8_lossy(e.as_ref());
                    resolve_predefined_entity(&name)
                };
                if let Some(c) = ch {
                    let mut buf_char = [0u8; 4];
                    let s = c.encode_utf8(&mut buf_char);
                    text_buf.push_str(s);
                }
            }
            Event::End(ref e) if local_name(e.name().as_ref()) == b"text" => {
                if let Some(c) = current.as_mut() {
                    c.text = text_buf.trim().to_string();
                }
                in_text = false;
            }
            Event::End(ref e) if local_name(e.name().as_ref()) == b"threadedComment" => {
                if let Some(c) = current.take()
                    && !c.text.is_empty()
                {
                    let author = c
                        .person_id
                        .as_deref()
                        .and_then(|pid| persons.get(pid).cloned())
                        .filter(|s| !s.is_empty());
                    comments.push(ParsedComment {
                        row: c.row,
                        col: c.col,
                        author,
                        text: c.text,
                        parent_id: c.parent_id,
                    });
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(comments)
}

struct PendingThreaded {
    row: u32,
    col: u32,
    person_id: Option<String>,
    parent_id: Option<String>,
    text: String,
}

// ---------------------------------------------------------------------------
// Workbook-level person resolution (xl/persons/person{N}.xml)
// ---------------------------------------------------------------------------

/// Walk the workbook relationships once to collect every person file and
/// resolve `personId → displayName`.
///
/// Person parts are referenced from `xl/_rels/workbook.xml.rels` with rel
/// type `…/officeDocument/2017/10/relationships/person`. Each part's root
/// is `<personList>` with `<person id="…" displayName="…"/>` children.
fn load_workbook_persons(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    warnings: &mut Vec<Warning>,
) -> HashMap<String, String> {
    let mut people = HashMap::new();
    let workbook_rels_path = relationships_path_for_part("xl/workbook.xml");
    let Some(rels_xml) = read_zip_entry_raw(archive, &workbook_rels_path, None, warnings) else {
        return people;
    };
    let person_targets = match parse_relationship_targets(&rels_xml, Some("person")) {
        Ok(targets) => targets,
        Err(e) => {
            warnings.push(Warning {
                kind: WarningKind::MalformedContent,
                message: format!("Failed to scan workbook relationships for persons: {}", e),
                page: None,
            });
            return people;
        }
    };
    for target in person_targets.values() {
        let path = resolve_relationship_target("xl/workbook.xml", target);
        let Some(xml) = read_zip_entry_raw(archive, &path, None, warnings) else {
            continue;
        };
        match parse_person_list(&xml) {
            Ok(parsed) => {
                for (id, name) in parsed {
                    people.insert(id, name);
                }
            }
            Err(e) => warnings.push(Warning {
                kind: WarningKind::MalformedContent,
                message: format!("Failed to parse person list at '{}': {}", path, e),
                page: None,
            }),
        }
    }
    people
}

fn parse_person_list(xml: &[u8]) -> Result<Vec<(String, String)>, quick_xml::Error> {
    let mut reader = XmlReader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut out = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(ref e) | Event::Empty(ref e)
                if local_name(e.name().as_ref()) == b"person" =>
            {
                let mut id = None;
                let mut display_name = None;
                for attr in e.attributes().flatten() {
                    match local_name(attr.key.as_ref()).as_slice() {
                        b"id" => id = Some(attr_value(&attr)),
                        b"displayName" => display_name = Some(attr_value(&attr)),
                        _ => {}
                    }
                }
                if let (Some(id), Some(name)) = (id, display_name) {
                    out.push((id, name));
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_single_comment_with_author() {
        let xml = br#"<?xml version="1.0"?>
<comments xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <authors>
    <author>Alice</author>
  </authors>
  <commentList>
    <comment ref="C6" authorId="0">
      <text><r><t>Overrun due to unexpected Phase II trial site in Halifax</t></r></text>
    </comment>
  </commentList>
</comments>"#;
        let comments = parse_legacy_comments(xml).unwrap();
        assert_eq!(comments.len(), 1);
        let c = &comments[0];
        // C6 → row 5, col 2 (0-indexed)
        assert_eq!((c.row, c.col), (5, 2));
        assert_eq!(c.author.as_deref(), Some("Alice"));
        assert_eq!(
            c.text,
            "Overrun due to unexpected Phase II trial site in Halifax"
        );
    }

    #[test]
    fn legacy_rich_text_with_rpr_extracts_text() {
        // Regression: Office-authored comment files use the rich-text shape
        // `<r><rPr>…</rPr><t xml:space="preserve">…</t></r>` inside `<text>`.
        // We must walk past `<rPr>` without losing the `<text>` / `<t>`
        // enclosing state, or the comment surfaces as an empty string.
        let xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<comments xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <authors><author>Unknown Author</author></authors>
  <commentList>
    <comment ref="C6" authorId="0">
      <text>
        <r>
          <rPr><sz val="10"/><rFont val="Arial"/><family val="2"/></rPr>
          <t xml:space="preserve">Overrun due to unexpected Phase II trial site in Halifax. See Details sheet row 22.</t>
        </r>
      </text>
    </comment>
  </commentList>
</comments>"#;
        let comments = parse_legacy_comments(xml).unwrap();
        assert_eq!(comments.len(), 1);
        let c = &comments[0];
        assert_eq!((c.row, c.col), (5, 2));
        assert_eq!(
            c.text,
            "Overrun due to unexpected Phase II trial site in Halifax. See Details sheet row 22."
        );
    }

    #[test]
    fn render_comment_strips_placeholder_author() {
        let c = ParsedComment {
            row: 5,
            col: 2,
            author: Some("Unknown Author".to_string()),
            text: "Overrun due to unexpected Phase II trial site in Halifax".to_string(),
            parent_id: None,
        };
        assert_eq!(
            render_comment(&c),
            "Overrun due to unexpected Phase II trial site in Halifax"
        );
    }

    #[test]
    fn legacy_downgrade_stub_recognized() {
        assert!(is_threaded_downgrade_stub(
            "[Threaded comment] Your version of Excel allows you to read this threaded comment …"
        ));
        assert!(!is_threaded_downgrade_stub(
            "Overrun due to unexpected Phase II trial site in Halifax"
        ));
    }

    #[test]
    fn threaded_single_comment_resolves_author_via_persons() {
        let mut persons = HashMap::new();
        persons.insert(
            "{B75190B2-7DA9-42A6-BCA2-25F384155279}".to_string(),
            "Mark Baker".to_string(),
        );
        let xml = br#"<?xml version="1.0"?>
<ThreadedComments xmlns="http://schemas.microsoft.com/office/spreadsheetml/2018/threadedcomments">
  <threadedComment ref="C6" dT="2026-04-01T10:00:00Z" personId="{B75190B2-7DA9-42A6-BCA2-25F384155279}" id="{1111}">
    <text>Overrun due to unexpected Phase II trial site in Halifax</text>
  </threadedComment>
</ThreadedComments>"#;
        let comments = parse_threaded_comments(xml, &persons).unwrap();
        assert_eq!(comments.len(), 1);
        let c = &comments[0];
        assert_eq!((c.row, c.col), (5, 2));
        assert_eq!(c.author.as_deref(), Some("Mark Baker"));
        assert_eq!(
            c.text,
            "Overrun due to unexpected Phase II trial site in Halifax"
        );
    }

    #[test]
    fn threaded_thread_groups_into_reply_chain() {
        let mut persons = HashMap::new();
        persons.insert("p1".to_string(), "Alice".to_string());
        persons.insert("p2".to_string(), "Bob".to_string());
        let xml = br#"<?xml version="1.0"?>
<ThreadedComments xmlns="http://schemas.microsoft.com/office/spreadsheetml/2018/threadedcomments">
  <threadedComment ref="A1" personId="p1" id="root">
    <text>Why is this number so high?</text>
  </threadedComment>
  <threadedComment ref="A1" personId="p2" id="reply" parentId="root">
    <text>It is correct, see the supporting note.</text>
  </threadedComment>
</ThreadedComments>"#;
        let comments = parse_threaded_comments(xml, &persons).unwrap();
        let groups = group_into_threads(comments);
        assert_eq!(groups.len(), 1);
        let ((row, col), text) = &groups[0];
        assert_eq!((*row, *col), (0, 0));
        assert!(text.starts_with("Alice: Why is this number so high?"));
        assert!(text.contains("\n  ↳ Bob: It is correct, see the supporting note."));
    }

    #[test]
    fn person_list_parses_displayname() {
        let xml = br#"<?xml version="1.0"?>
<personList xmlns="http://schemas.microsoft.com/office/spreadsheetml/2018/threadedcomments">
  <person providerId="Windows Live" userId="abc" id="{ID-1}" displayName="Mark Baker"/>
</personList>"#;
        let persons = parse_person_list(xml).unwrap();
        assert_eq!(persons.len(), 1);
        assert_eq!(persons[0].0, "{ID-1}");
        assert_eq!(persons[0].1, "Mark Baker");
    }
}
