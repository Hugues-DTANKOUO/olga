use std::collections::BTreeMap;
use std::io::Cursor;

use crate::error::{Warning, WarningKind};

use super::super::body::parse_body_xml;
use super::super::rels::{collect_media, parse_rels, read_zip_entry};
use super::super::types::{
    DocxPackageIssueCounts, NumberingLevel, PartContext, RelType, Relationship, ResolvedStyle,
};
use super::super::{PATH_DOCUMENT_XML, PATH_WORD_PREFIX};
use super::PATH_DOCUMENT2_XML;

const PATH_DOC_RELS: &str = "word/_rels/document.xml.rels";
const PATH_DOC2_RELS: &str = "word/_rels/document2.xml.rels";
const PATH_WORD_RELS_PREFIX: &str = "word/_rels/";
const PATH_RELS_SUFFIX: &str = ".rels";

pub(super) fn resolve_document_path(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
) -> Option<&'static str> {
    if archive.by_name(PATH_DOCUMENT_XML).is_ok() {
        Some(PATH_DOCUMENT_XML)
    } else if archive.by_name(PATH_DOCUMENT2_XML).is_ok() {
        Some(PATH_DOCUMENT2_XML)
    } else {
        None
    }
}

pub(super) fn rels_path_for_document(doc_path: &str) -> &'static str {
    match doc_path {
        PATH_DOCUMENT_XML => PATH_DOC_RELS,
        PATH_DOCUMENT2_XML => PATH_DOC2_RELS,
        _ => PATH_DOC_RELS,
    }
}

fn secondary_part_label(part_ctx: PartContext) -> &'static str {
    match part_ctx {
        PartContext::Header => "header parts",
        PartContext::Footer => "footer parts",
        PartContext::Footnotes => "footnote parts",
        PartContext::Endnotes => "endnote parts",
        PartContext::Comments => "comment parts",
        PartContext::Body => "secondary parts",
    }
}

pub(super) fn append_docx_package_infrastructure_summaries(
    warnings: &mut Vec<Warning>,
    package_issue_counts: &DocxPackageIssueCounts,
) {
    if package_issue_counts.missing_media_payloads > 1 {
        warnings.push(Warning {
            kind: WarningKind::MissingMedia,
            message: format!(
                "DOCX package contains {} missing media payloads referenced by relationship parts",
                package_issue_counts.missing_media_payloads
            ),
            page: None,
        });
    }

    if package_issue_counts.broken_numbering_defs > 1 {
        warnings.push(Warning {
            kind: WarningKind::PartialExtraction,
            message: format!(
                "DOCX numbering.xml contains {} numbering definitions referencing missing abstract numbering entries",
                package_issue_counts.broken_numbering_defs
            ),
            page: None,
        });
    }

    if package_issue_counts.malformed_relationship_parts > 1 {
        warnings.push(Warning {
            kind: WarningKind::MalformedContent,
            message: format!(
                "DOCX package contains {} malformed relationship parts under word/_rels/",
                package_issue_counts.malformed_relationship_parts
            ),
            page: None,
        });
    }

    if package_issue_counts.missing_style_definitions > 1 {
        warnings.push(Warning {
            kind: WarningKind::UnresolvedStyle,
            message: format!(
                "DOCX styles.xml contains {} unresolved style references in inheritance chains",
                package_issue_counts.missing_style_definitions
            ),
            page: None,
        });
    }

    if package_issue_counts.circular_style_chains > 1 {
        warnings.push(Warning {
            kind: WarningKind::UnresolvedStyle,
            message: format!(
                "DOCX styles.xml contains {} style inheritance chains exceeding the recursion limit",
                package_issue_counts.circular_style_chains
            ),
            page: None,
        });
    }

    if package_issue_counts.malformed_auxiliary_xml_files > 1 {
        warnings.push(Warning {
            kind: WarningKind::MalformedContent,
            message: format!(
                "DOCX package contains {} malformed auxiliary XML files among styles.xml, numbering.xml, or docProps/app.xml",
                package_issue_counts.malformed_auxiliary_xml_files
            ),
            page: None,
        });
    }

    if package_issue_counts.malformed_secondary_story_xml_parts > 1 {
        warnings.push(Warning {
            kind: WarningKind::UnexpectedStructure,
            message: format!(
                "DOCX package contains {} malformed secondary story XML parts; inspect story-level XML parse warnings for detail",
                package_issue_counts.malformed_secondary_story_xml_parts
            ),
            page: None,
        });
    }
}

/// Deterministic sort key for secondary DOCX parts.
///
/// Uses the same ordering convention as mainstream DOCX extractors
/// (Apache POI's `XWPFWordExtractor`, python-docx, Mammoth): headers
/// first → body → footers → footnotes/endnotes → comments — so the tail
/// of the extracted document reads consistently regardless of how the
/// authoring application laid out `word/_rels/document.xml.rels`.
/// Sorting is required because `doc_rels` is a `HashMap`, whose iteration
/// order is randomized per-process — without a stable order, footer
/// primitives picked up `source_order` values that occasionally
/// interleaved with body table headers in the markdown renderer (most
/// visibly on memo.docx, where the page footer row would overwrite the
/// approval-matrix header cells "Expense category" and "Up to $500").
fn secondary_rel_order(rel_type: &RelType) -> u8 {
    match rel_type {
        RelType::Header => 0,
        RelType::Footer => 1,
        RelType::Footnotes => 2,
        RelType::Endnotes => 3,
        RelType::Comments => 4,
        _ => 99,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn process_secondary_parts(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    doc_rels: &std::collections::HashMap<String, Relationship>,
    styles: &std::collections::HashMap<String, ResolvedStyle>,
    numbering: &std::collections::HashMap<(String, u8), NumberingLevel>,
    primitives: &mut Vec<crate::model::Primitive>,
    source_order: &mut u64,
    warnings: &mut Vec<Warning>,
    package_issue_counts: &mut DocxPackageIssueCounts,
) {
    // Secondary DOCX parts (header*.xml, footer*.xml, footnotes.xml, ...)
    // are page chrome, not part of the linear reading flow of word/document.xml.
    // DOCX has no pre-rendered pages: `word/footer1.xml` is a *template*
    // referenced by `sectPr`, not an instance bound to page N. Mainstream
    // structure-driven extractors therefore treat them specially:
    //
    //   * Apache POI XWPFWordExtractor emits headers-then-body-then-footers
    //     (https://poi.apache.org/apidocs/dev/org/apache/poi/xwpf/extractor/XWPFWordExtractor.html).
    //   * python-docx exposes them on a separate `Section.header` / `.footer`
    //     object graph rather than in `Document.paragraphs`.
    //   * pandoc and mammoth drop them entirely by default
    //     (https://github.com/jgm/pandoc/issues/5211).
    //
    // olga's choice: preserve the content (like POI) but segregate it from
    // the body coordinate space so it cannot collide with body tables under
    // any HashMap iteration order. We pin every secondary primitive to the
    // last body page and push its `y` past the lowest body primitive on
    // that page, then advance the offset between parts so they stack rather
    // than overwrite each other. The PageHeader/PageFooter hint attached by
    // `attach_docx_media_structural_hints` is preserved for downstream
    // filtering.
    let last_body_page = primitives.iter().map(|p| p.page).max().unwrap_or(0);
    let body_max_y_on_last_page = primitives
        .iter()
        .filter(|p| p.page == last_body_page)
        .map(|p| p.bbox.y + p.bbox.height)
        .fold(0.0f32, f32::max);

    // Sort rels deterministically. See `secondary_rel_order` for rationale.
    let mut ordered_rels: Vec<&Relationship> = doc_rels.values().collect();
    ordered_rels.sort_by(|a, b| {
        secondary_rel_order(&a.rel_type)
            .cmp(&secondary_rel_order(&b.rel_type))
            .then_with(|| a.target.cmp(&b.target))
    });

    // Start the first secondary part a full unit past the last body primitive
    // so neither the prim_spatial renderer (source_order-sorted) nor the
    // assembler (bbox-clustered) merges chrome with body.
    let mut secondary_part_y = body_max_y_on_last_page + 1.0;

    let mut missing_secondary_part_summaries: BTreeMap<&'static str, u32> = BTreeMap::new();
    for rel in ordered_rels {
        let part_ctx = match &rel.rel_type {
            RelType::Header => PartContext::Header,
            RelType::Footer => PartContext::Footer,
            RelType::Footnotes => PartContext::Footnotes,
            RelType::Endnotes => PartContext::Endnotes,
            RelType::Comments => PartContext::Comments,
            _ => continue,
        };

        let path = format!("{}{}", PATH_WORD_PREFIX, rel.target);
        let rels_path = format!(
            "{}{}{}",
            PATH_WORD_RELS_PREFIX, rel.target, PATH_RELS_SUFFIX
        );
        let part_rels = parse_rels(archive, &rels_path, warnings, false, package_issue_counts);
        let part_media = collect_media(archive, &part_rels, warnings, package_issue_counts);

        match read_zip_entry(archive, &path) {
            Ok(xml) => {
                let mut part_y = secondary_part_y;
                let mut part_page = last_body_page;
                if parse_body_xml(
                    &xml,
                    styles,
                    numbering,
                    &part_rels,
                    &part_media,
                    primitives,
                    source_order,
                    &mut part_page,
                    &mut part_y,
                    warnings,
                    part_ctx,
                ) {
                    package_issue_counts.malformed_secondary_story_xml_parts += 1;
                }
                // Stack the next secondary part past the one we just emitted
                // so consecutive parts never collide with each other either.
                secondary_part_y = part_y + 1.0;
            }
            Err(_) => {
                warnings.push(Warning {
                    kind: WarningKind::MissingPart,
                    message: format!("Secondary DOCX part missing: {}", path),
                    page: None,
                });
                *missing_secondary_part_summaries
                    .entry(secondary_part_label(part_ctx))
                    .or_default() += 1;
            }
        }
    }

    for (part_label, count) in &missing_secondary_part_summaries {
        if *count > 1 {
            warnings.push(Warning {
                kind: WarningKind::MissingPart,
                message: format!(
                    "DOCX package references {} missing {}; related secondary story content could not be decoded",
                    count, part_label
                ),
                page: None,
            });
        }
    }
}
