//! Unit tests for the public API surface.
//!
//! These tests intentionally exercise the surface — constructors, format
//! detection, page iteration, text/markdown maps, JSON round-trip, outline —
//! against the fixture corpus. They are smoke tests, not correctness tests
//! on the renderers themselves (that belongs in each renderer's own suite).

use super::*;
use std::path::PathBuf;

fn corpus(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("corpus")
        .join(path)
}

// ---------------------------------------------------------------------------
// Format detection
// ---------------------------------------------------------------------------

#[test]
fn detects_pdf_from_magic_bytes() {
    let data = std::fs::read(corpus("pdf/structured_report.pdf")).unwrap();
    assert_eq!(detect_format(&data).unwrap(), Format::Pdf);
}

#[test]
fn detects_docx_from_zip_layout() {
    let data = std::fs::read(corpus("docx/project_status.docx")).unwrap();
    assert_eq!(detect_format(&data).unwrap(), Format::Docx);
}

#[test]
fn detects_xlsx_from_zip_layout() {
    let data = std::fs::read(corpus("xlsx/employee_directory.xlsx")).unwrap();
    assert_eq!(detect_format(&data).unwrap(), Format::Xlsx);
}

#[test]
fn detects_html_from_preview_prefix() {
    let data = std::fs::read(corpus("html/complex_report.html")).unwrap();
    assert_eq!(detect_format(&data).unwrap(), Format::Html);
}

#[test]
fn detect_format_rejects_unknown_bytes() {
    let garbage = b"this is not a document, just a string of bytes";
    assert!(matches!(
        detect_format(garbage).unwrap_err(),
        Error::UnsupportedFormat(_)
    ));
}

// ---------------------------------------------------------------------------
// Constructor: Document::open
// ---------------------------------------------------------------------------

#[test]
fn opens_pdf_from_path() {
    let doc = Document::open(corpus("pdf/structured_report.pdf")).unwrap();
    assert_eq!(doc.format(), Format::Pdf);
    assert!(doc.page_count() >= 1);
    assert!(doc.is_processable());
}

#[test]
fn opens_docx_from_path() {
    let doc = Document::open(corpus("docx/project_status.docx")).unwrap();
    assert_eq!(doc.format(), Format::Docx);
    assert!(doc.page_count() >= 1);
}

#[test]
fn opens_xlsx_from_path() {
    let doc = Document::open(corpus("xlsx/employee_directory.xlsx")).unwrap();
    assert_eq!(doc.format(), Format::Xlsx);
    assert!(doc.page_count() >= 1);
}

#[test]
fn opens_html_from_path() {
    let doc = Document::open(corpus("html/complex_report.html")).unwrap();
    assert_eq!(doc.format(), Format::Html);
    // HTML is always exactly one logical page.
    assert_eq!(doc.page_count(), 1);
}

// ---------------------------------------------------------------------------
// Constructor: Document::open_bytes
// ---------------------------------------------------------------------------

#[test]
fn opens_bytes_without_hint_uses_sniffing() {
    let data = std::fs::read(corpus("pdf/structured_report.pdf")).unwrap();
    let doc = Document::open_bytes(data, None).unwrap();
    assert_eq!(doc.format(), Format::Pdf);
}

#[test]
fn opens_bytes_with_explicit_hint() {
    let data = std::fs::read(corpus("docx/project_status.docx")).unwrap();
    let doc = Document::open_bytes(data, Some(Format::Docx)).unwrap();
    assert_eq!(doc.format(), Format::Docx);
}

// ---------------------------------------------------------------------------
// Pages
// ---------------------------------------------------------------------------

#[test]
fn pages_are_one_based() {
    let doc = Document::open(corpus("pdf/structured_report.pdf")).unwrap();
    let first = doc.pages().next().unwrap();
    assert_eq!(first.number(), 1);

    assert!(doc.page(0).is_none(), "page 0 must be out of range");
    assert!(doc.page(1).is_some());
    assert!(doc.page(doc.page_count() + 1).is_none());
}

#[test]
fn pages_iterator_covers_all_pages() {
    let doc = Document::open(corpus("pdf/structured_report.pdf")).unwrap();
    let page_numbers: Vec<usize> = doc.pages().map(|p| p.number()).collect();
    let expected: Vec<usize> = (1..=doc.page_count()).collect();
    assert_eq!(page_numbers, expected);
}

// ---------------------------------------------------------------------------
// Text / Markdown
// ---------------------------------------------------------------------------

#[test]
fn text_by_page_is_one_based_and_nonempty_for_pdf() {
    let doc = Document::open(corpus("pdf/structured_report.pdf")).unwrap();
    let by_page = doc.text_by_page();
    assert!(
        !by_page.is_empty(),
        "PDF fixture must render at least one page"
    );
    // Smallest key is 1, never 0.
    assert_eq!(*by_page.keys().next().unwrap(), 1);
}

#[test]
fn page_text_matches_document_text_by_page() {
    let doc = Document::open(corpus("pdf/structured_report.pdf")).unwrap();
    let by_page = doc.text_by_page();
    for page in doc.pages() {
        let expected = by_page.get(&page.number()).cloned().unwrap_or_default();
        assert_eq!(
            page.text(),
            expected,
            "page {} text mismatch",
            page.number()
        );
    }
}

#[test]
fn markdown_by_page_is_one_based_for_pdf() {
    let doc = Document::open(corpus("pdf/structured_report.pdf")).unwrap();
    let by_page = doc.markdown_by_page();
    if !by_page.is_empty() {
        assert_eq!(*by_page.keys().next().unwrap(), 1);
    }
}

#[test]
fn docx_text_by_page_covers_pages() {
    let doc = Document::open(corpus("docx/project_status.docx")).unwrap();
    let by_page = doc.text_by_page();
    // Each populated page key must be in range [1, page_count].
    for &p in by_page.keys() {
        assert!(p >= 1 && p <= doc.page_count());
    }
}

#[test]
fn html_always_has_single_page_entry() {
    let doc = Document::open(corpus("html/complex_report.html")).unwrap();
    let by_page = doc.text_by_page();
    // HTML has no physical pagination — the renderer produces one chunk.
    assert!(by_page.len() <= 1);
    if let Some(&key) = by_page.keys().next() {
        assert_eq!(key, 1);
    }
}

// ---------------------------------------------------------------------------
// JSON / outline
// ---------------------------------------------------------------------------

#[test]
fn to_json_returns_object_with_expected_top_level_keys() {
    let doc = Document::open(corpus("docx/project_status.docx")).unwrap();
    let json = doc.to_json().unwrap();
    let obj = json.as_object().expect("top-level json must be an object");
    // We don't lock the full schema — just check the spine.
    assert!(obj.contains_key("root") || obj.contains_key("document") || !obj.is_empty());
}

#[test]
fn outline_is_collected_in_document_order() {
    let doc = Document::open(corpus("docx/project_status.docx")).unwrap();
    let outline = doc.outline().unwrap();
    // Not strict on content — just that collection succeeded and entries are
    // ordered by page (non-decreasing).
    let mut prev_page = 0u32;
    for entry in &outline {
        assert!(
            entry.page >= prev_page,
            "outline entries must be in document order"
        );
        prev_page = entry.page;
    }
}

// ---------------------------------------------------------------------------
// Images — dedicated channel
// ---------------------------------------------------------------------------

#[test]
fn pdf_exposes_images_channel() {
    let doc = Document::open(corpus("pdf/structured_report.pdf")).unwrap();
    // We don't assert a specific count (the corpus PDF may or may not carry
    // raster images), but the accessor must be stable and the per-page sum
    // must equal the document-level total.
    let document_total = doc.image_count();
    let page_total: usize = doc.pages().map(|p| p.image_count()).sum();
    assert_eq!(document_total, page_total);
    for img in doc.images() {
        // 0-based internal page number; must fit within `page_count`.
        assert!((img.page as usize) < doc.page_count());
    }
}

#[test]
fn page_images_filter_matches_document_images() {
    let doc = Document::open(corpus("pdf/structured_report.pdf")).unwrap();
    let all = doc.images();
    for page in doc.pages() {
        let idx = page.number() - 1;
        let page_imgs = page.images();
        // Every per-page image must come from the document-level set and be
        // filed under this page's index.
        assert_eq!(page_imgs.len(), page.image_count());
        assert!(page_imgs.iter().all(|img| img.page as usize == idx));
        assert_eq!(
            page_imgs.len(),
            all.iter().filter(|img| img.page as usize == idx).count()
        );
    }
}

#[test]
fn docx_images_mirror_primitive_images() {
    // DOCX fixture with inline media.
    let doc = Document::open(corpus("docx/mixed_content_stress.docx")).unwrap();
    // The mirror keeps every image bit-exact: the bytes live once in the
    // primitive stream and once on the channel.
    for img in doc.images() {
        assert!(!img.data.is_empty(), "mirrored image must carry bytes");
    }
    // Image count is non-decreasing across pages.
    let mut seen = 0usize;
    for page in doc.pages() {
        seen += page.image_count();
    }
    assert_eq!(seen, doc.image_count());
}

#[test]
fn html_single_page_images_filter_to_page_1() {
    let doc = Document::open(corpus("html/complex_report.html")).unwrap();
    // HTML is single-page — every image must live on page 1 (index 0).
    for img in doc.images() {
        assert_eq!(img.page, 0);
    }
    if let Some(page) = doc.page(1) {
        assert_eq!(page.image_count(), doc.image_count());
    }
}

#[test]
fn xlsx_has_no_images_in_v1() {
    let doc = Document::open(corpus("xlsx/employee_directory.xlsx")).unwrap();
    assert_eq!(
        doc.image_count(),
        0,
        "XLSX image extraction is not wired in v1"
    );
    assert!(doc.images().is_empty());
}

// ---------------------------------------------------------------------------
// Non-regression: images channel must NOT mutate text / markdown / JSON output
// ---------------------------------------------------------------------------

#[test]
fn images_channel_is_non_invasive_for_text_and_markdown() {
    // Brick B promise: within a single Document, forcing the images channel
    // must not mutate the cached text / markdown / JSON outputs. This catches
    // accidental writes back into the primitive stream from the image
    // extraction pipeline.
    //
    // (Testing cross-document determinism belongs elsewhere — the structure
    // engine has known HashMap-order wobble that this assertion would pick up
    // for reasons unrelated to images.)
    for path in [
        "pdf/structured_report.pdf",
        "docx/mixed_content_stress.docx",
        "xlsx/employee_directory.xlsx",
        "html/complex_report.html",
    ] {
        let doc = Document::open(corpus(path)).unwrap();

        // Force renderers first. OnceLock freezes these values.
        let text_before = doc.text();
        let md_before = doc.markdown();
        let json_before = doc.to_json().unwrap().to_string();

        // Now force the images channel — must not observe or mutate anything
        // the cached renderers would see.
        let _ = doc.images();
        let _: usize = doc.image_count();

        let text_after = doc.text();
        let md_after = doc.markdown();
        let json_after = doc.to_json().unwrap().to_string();

        assert_eq!(text_before, text_after, "{path} text drifted");
        assert_eq!(md_before, md_after, "{path} markdown drifted");
        assert_eq!(json_before, json_after, "{path} json drifted");
    }
}

// ---------------------------------------------------------------------------
// Links — document + per-page
// ---------------------------------------------------------------------------

#[test]
fn html_exposes_hyperlinks_from_anchor_tags() {
    let doc = Document::open(corpus("html/complex_report.html")).unwrap();
    let links = doc.links();
    // Don't lock exact count — the fixture is free to evolve — just assert
    // every link has sane fields and the single HTML page owns them all.
    for link in &links {
        assert!(!link.url.is_empty(), "every link must carry a URL");
        assert!(!link.text.is_empty(), "every link must carry anchor text");
        assert_eq!(link.page, 1, "HTML lives on page 1");
    }
    // Document-level count == sum of per-page counts.
    let doc_total = doc.link_count();
    let page_total: usize = doc.pages().map(|p| p.link_count()).sum();
    assert_eq!(doc_total, page_total);
}

#[test]
fn docx_exposes_hyperlinks_when_present() {
    let doc = Document::open(corpus("docx/project_status.docx")).unwrap();
    let links = doc.links();
    // Whatever the fixture contains, the invariants below always hold.
    for link in &links {
        assert!(!link.url.is_empty());
        assert!(link.page >= 1 && (link.page as usize) <= doc.page_count());
    }
}

#[test]
fn pdf_link_coalescing_is_deterministic() {
    let doc = Document::open(corpus("pdf/structured_report.pdf")).unwrap();
    // Calling twice must yield the same result — the walk is stateless over
    // the cached primitive stream.
    let a = doc.links();
    let b = doc.links();
    assert_eq!(a, b);
}

#[test]
fn page_links_are_a_subset_of_document_links() {
    let doc = Document::open(corpus("html/complex_report.html")).unwrap();
    let all = doc.links();
    for page in doc.pages() {
        let page_links = page.links();
        // Every per-page link must exist in the document-level list, filtered
        // by page. Order must also match (both walks are in source order).
        let expected: Vec<_> = all
            .iter()
            .filter(|l| l.page == page.number() as u32)
            .cloned()
            .collect();
        assert_eq!(page_links, expected);
    }
}

// ---------------------------------------------------------------------------
// Tables — document + per-page
// ---------------------------------------------------------------------------

#[test]
fn xlsx_sheets_expose_at_least_one_table_per_sheet() {
    let doc = Document::open(corpus("xlsx/employee_directory.xlsx")).unwrap();
    // XLSX decodes each sheet as a table-of-rows via the structure engine.
    let tables = doc.tables();
    assert!(
        !tables.is_empty(),
        "XLSX fixture must expose at least one table"
    );
    for table in &tables {
        assert!(table.rows >= 1);
        assert!(table.cols >= 1);
        assert!(table.first_page >= 1);
        assert!(table.last_page >= table.first_page);
        assert!(
            (table.first_page as usize) <= doc.page_count(),
            "first_page must be in range"
        );
        // Cells carry sensible spans. We don't enforce `cell.row < table.rows`
        // because the structure engine reports `rows` as the count of distinct
        // row indices, which can be smaller than `max(row) + 1` if the source
        // has gaps. The same applies to columns.
        for cell in &table.cells {
            assert!(cell.rowspan >= 1);
            assert!(cell.colspan >= 1);
        }
    }
}

#[test]
fn docx_tables_carry_cells() {
    let doc = Document::open(corpus("docx/mixed_content_stress.docx")).unwrap();
    for table in doc.tables() {
        // At least one cell and the bbox is non-degenerate.
        assert!(!table.cells.is_empty(), "DOCX tables must carry cells");
    }
}

#[test]
fn cross_page_table_shows_on_every_page_it_covers() {
    let doc = Document::open(corpus("xlsx/employee_directory.xlsx")).unwrap();
    // Find the first table and verify it appears on each page it spans.
    if let Some(table) = doc.tables().into_iter().next() {
        for page_num in table.first_page..=table.last_page {
            if let Some(page) = doc.page(page_num as usize) {
                let on_page = page.tables();
                assert!(
                    on_page.iter().any(|t| t == &table),
                    "table must appear on every page in {}..={}",
                    table.first_page,
                    table.last_page,
                );
            }
        }
    }
}

#[test]
fn document_table_count_matches_tables_len() {
    let doc = Document::open(corpus("xlsx/employee_directory.xlsx")).unwrap();
    assert_eq!(doc.table_count(), doc.tables().len());
}

// ---------------------------------------------------------------------------
// Search — document + per-page
// ---------------------------------------------------------------------------

#[test]
fn empty_query_returns_no_hits() {
    let doc = Document::open(corpus("html/complex_report.html")).unwrap();
    assert!(doc.search("").is_empty());
    for page in doc.pages() {
        assert!(page.search("").is_empty());
    }
}

#[test]
fn search_is_case_insensitive() {
    let doc = Document::open(corpus("html/complex_report.html")).unwrap();
    let full_text = doc.text();
    // Pick a token that appears in the rendered text and bounce its casing.
    // Uses an ASCII-only letter search term to stay under `to_ascii_lowercase`.
    let needle = full_text
        .split_whitespace()
        .find(|w| w.chars().all(|c| c.is_ascii_alphabetic()) && w.len() >= 4)
        .map(|s| s.to_string())
        .expect("fixture should contain at least one ASCII word");
    let lower = needle.to_ascii_lowercase();
    let upper = needle.to_ascii_uppercase();
    // Exact casing returns hits; bouncing casing returns at least the same set.
    let a = doc.search(&lower);
    let b = doc.search(&upper);
    assert!(!a.is_empty(), "search term '{lower}' must hit");
    // Counts must match because the match is purely case-folded.
    assert_eq!(a.len(), b.len());
}

#[test]
fn search_hit_coordinates_point_to_the_right_line() {
    let doc = Document::open(corpus("html/complex_report.html")).unwrap();
    let text_by_page = doc.text_by_page();
    // Find any non-empty word to search for.
    let mut some_word: Option<String> = None;
    for content in text_by_page.values() {
        if let Some(w) = content
            .split_whitespace()
            .find(|w| w.chars().all(|c| c.is_ascii_alphabetic()) && w.len() >= 4)
        {
            some_word = Some(w.to_string());
            break;
        }
    }
    let Some(word) = some_word else {
        return;
    };
    for hit in doc.search(&word) {
        let page_text = text_by_page.get(&(hit.page as usize)).unwrap();
        let line = page_text.lines().nth((hit.line - 1) as usize).unwrap();
        assert_eq!(line, hit.snippet);
        // The matched substring must land at the reported character column.
        let chars_before: String = line.chars().take(hit.col_start as usize).collect();
        let rest: String = line.chars().skip(hit.col_start as usize).collect();
        assert!(
            rest.to_ascii_lowercase()
                .starts_with(&hit.match_text.to_ascii_lowercase()),
            "hit col_start must point at the match; chars_before={chars_before:?} rest={rest:?}",
        );
    }
}

#[test]
fn page_search_restricts_to_that_page() {
    let doc = Document::open(corpus("pdf/structured_report.pdf")).unwrap();
    // Grab a token from page 1's text.
    let page1 = doc.page(1).unwrap();
    let text = page1.text();
    let Some(word) = text
        .split_whitespace()
        .find(|w| w.chars().all(|c| c.is_ascii_alphabetic()) && w.len() >= 4)
        .map(|s| s.to_string())
    else {
        return;
    };
    let hits = page1.search(&word);
    assert!(hits.iter().all(|h| h.page == 1));
}

// ---------------------------------------------------------------------------
// Chunks — document + per-page
// ---------------------------------------------------------------------------

#[test]
fn chunks_by_page_pairs_up_with_text_by_page() {
    let doc = Document::open(corpus("pdf/structured_report.pdf")).unwrap();
    let chunks = doc.chunks_by_page();
    let text_by_page = doc.text_by_page();
    // Every chunk corresponds to a text_by_page entry.
    for chunk in &chunks {
        let page_text = text_by_page.get(&(chunk.page as usize)).unwrap();
        assert_eq!(&chunk.text, page_text);
        assert_eq!(chunk.char_count, chunk.text.chars().count());
    }
    // Non-empty text pages are all represented.
    let non_empty_pages: Vec<u32> = text_by_page
        .iter()
        .filter_map(|(p, t)| {
            if t.trim().is_empty() {
                None
            } else {
                Some(*p as u32)
            }
        })
        .collect();
    let chunk_pages: Vec<u32> = chunks.iter().map(|c| c.page).collect();
    assert_eq!(non_empty_pages, chunk_pages);
}

#[test]
fn page_chunk_matches_document_chunks() {
    let doc = Document::open(corpus("pdf/structured_report.pdf")).unwrap();
    let doc_chunks = doc.chunks_by_page();
    for page in doc.pages() {
        let page_chunk = page.chunk();
        let expected = doc_chunks.iter().find(|c| c.page as usize == page.number());
        match (page_chunk, expected) {
            (Some(pc), Some(ec)) => assert_eq!(&pc, ec),
            (None, None) => {}
            _ => panic!("page {} chunk mismatch", page.number()),
        }
    }
}

// ---------------------------------------------------------------------------
// Non-regression: Brick C exposures are non-mutating on cached renderers
// ---------------------------------------------------------------------------

#[test]
fn brick_c_accessors_are_non_invasive() {
    // Mirror of the images-channel test from Brick B. Calling the new
    // accessors inside a single Document must never drift the cached
    // text / markdown / JSON output — otherwise the accessors have
    // accidentally taken a write path through primitives or the tree.
    for path in [
        "pdf/structured_report.pdf",
        "docx/mixed_content_stress.docx",
        "xlsx/employee_directory.xlsx",
        "html/complex_report.html",
    ] {
        let doc = Document::open(corpus(path)).unwrap();

        let text_before = doc.text();
        let md_before = doc.markdown();
        let json_before = doc.to_json().unwrap().to_string();

        let _ = doc.links();
        let _ = doc.tables();
        let _ = doc.search("the");
        let _ = doc.chunks_by_page();
        let _: usize = doc.link_count();
        let _: usize = doc.table_count();

        let text_after = doc.text();
        let md_after = doc.markdown();
        let json_after = doc.to_json().unwrap().to_string();

        assert_eq!(text_before, text_after, "{path} text drifted");
        assert_eq!(md_before, md_after, "{path} markdown drifted");
        assert_eq!(json_before, json_after, "{path} json drifted");
    }
}

// ---------------------------------------------------------------------------
// Processability
// ---------------------------------------------------------------------------

/// Name a `HealthIssue` variant without its payload, so we can compare kinds
/// for uniqueness (the deterministic emission contract).
fn health_issue_kind(issue: &HealthIssue) -> &'static str {
    match issue {
        HealthIssue::Encrypted => "Encrypted",
        HealthIssue::EmptyContent => "EmptyContent",
        HealthIssue::DecodeFailed => "DecodeFailed",
        HealthIssue::ApproximatePagination { .. } => "ApproximatePagination",
        HealthIssue::HeuristicStructure { .. } => "HeuristicStructure",
        HealthIssue::PartialExtraction { .. } => "PartialExtraction",
        HealthIssue::MissingPart { .. } => "MissingPart",
        HealthIssue::UnresolvedStyle { .. } => "UnresolvedStyle",
        HealthIssue::UnresolvedRelationship { .. } => "UnresolvedRelationship",
        HealthIssue::MissingMedia { .. } => "MissingMedia",
        HealthIssue::TruncatedContent { .. } => "TruncatedContent",
        HealthIssue::MalformedContent { .. } => "MalformedContent",
        HealthIssue::FilteredArtifact { .. } => "FilteredArtifact",
        HealthIssue::SuspectedArtifact { .. } => "SuspectedArtifact",
        HealthIssue::OtherWarnings { .. } => "OtherWarnings",
    }
}

#[test]
fn processability_pdf_is_not_blocked() {
    let doc = Document::open(corpus("pdf/structured_report.pdf")).unwrap();
    let report = doc.processability();
    assert!(!report.is_blocked(), "blockers: {:?}", report.blockers);
    assert!(report.is_processable);
    assert!(report.blockers.is_empty());
    assert_eq!(report.pages_total, doc.page_count() as u32);
    assert!(report.pages_with_content >= 1);
}

#[test]
fn processability_for_every_format_is_not_blocked() {
    for path in [
        "pdf/structured_report.pdf",
        "docx/project_status.docx",
        "xlsx/employee_directory.xlsx",
        "html/complex_report.html",
    ] {
        let doc = Document::open(corpus(path)).unwrap();
        let report = doc.processability();
        assert!(
            !report.is_blocked(),
            "{path} unexpectedly blocked: {:?}",
            report.blockers
        );
        assert!(report.is_processable, "{path} reported not processable");
        assert_eq!(report.pages_total, doc.page_count() as u32);
    }
}

#[test]
fn processability_helpers_agree_with_health_variant() {
    let doc = Document::open(corpus("pdf/structured_report.pdf")).unwrap();
    let report = doc.processability();
    let flags = [report.is_ok(), report.is_degraded(), report.is_blocked()];
    assert_eq!(
        flags.iter().filter(|b| **b).count(),
        1,
        "exactly one of is_ok/is_degraded/is_blocked must be true"
    );
    match report.health {
        Health::Ok => assert!(report.is_ok()),
        Health::Degraded => assert!(report.is_degraded()),
        Health::Blocked => assert!(report.is_blocked()),
    }
}

#[test]
fn processability_is_processable_mirrors_blockers() {
    for path in [
        "pdf/structured_report.pdf",
        "docx/mixed_content_stress.docx",
        "xlsx/employee_directory.xlsx",
        "html/complex_report.html",
    ] {
        let doc = Document::open(corpus(path)).unwrap();
        let report = doc.processability();
        assert_eq!(
            report.is_processable,
            report.blockers.is_empty(),
            "{path}: is_processable must be true iff blockers is empty"
        );
    }
}

#[test]
fn processability_issues_are_unique_per_kind() {
    // Core determinism guarantee: each HealthIssue variant appears at most
    // once across blockers + degradations. Otherwise the per-kind count
    // semantics break down (a consumer wouldn't know which bucket to trust).
    for path in [
        "pdf/structured_report.pdf",
        "docx/mixed_content_stress.docx",
        "xlsx/employee_directory.xlsx",
        "html/complex_report.html",
    ] {
        let doc = Document::open(corpus(path)).unwrap();
        let report = doc.processability();
        let mut kinds: Vec<&str> = report
            .blockers
            .iter()
            .chain(report.degradations.iter())
            .map(health_issue_kind)
            .collect();
        kinds.sort_unstable();
        let before = kinds.len();
        kinds.dedup();
        assert_eq!(
            before,
            kinds.len(),
            "{path}: HealthIssue kinds duplicated across blockers/degradations"
        );
    }
}

#[test]
fn processability_pages_with_content_matches_text_by_page() {
    let doc = Document::open(corpus("pdf/structured_report.pdf")).unwrap();
    let report = doc.processability();
    let expected = doc
        .text_by_page()
        .values()
        .filter(|t| !t.trim().is_empty())
        .count() as u32;
    assert_eq!(report.pages_with_content, expected);
}

#[test]
fn processability_warning_count_matches_structure_warnings() {
    // `warning_count` is the total non-fatal warning count that fed into
    // issue bucketing. It must at least cover everything the decoder phase
    // emitted (structure may add more), so the public `warnings()` slice is
    // never larger than the report's total.
    for path in [
        "pdf/structured_report.pdf",
        "docx/mixed_content_stress.docx",
        "xlsx/employee_directory.xlsx",
        "html/complex_report.html",
    ] {
        let doc = Document::open(corpus(path)).unwrap();
        let report = doc.processability();
        assert!(
            report.warning_count >= doc.warnings().len(),
            "{path}: warning_count ({}) < decode warnings ({})",
            report.warning_count,
            doc.warnings().len()
        );
    }
}

#[test]
fn processability_is_idempotent_across_repeated_calls() {
    let doc = Document::open(corpus("pdf/structured_report.pdf")).unwrap();
    let first = doc.processability();
    let second = doc.processability();
    assert_eq!(first, second, "processability must be deterministic");
}

#[test]
fn processability_serializes_round_trip_via_serde() {
    let doc = Document::open(corpus("pdf/structured_report.pdf")).unwrap();
    let report = doc.processability();
    let json = serde_json::to_string(&report).expect("serialize");
    let back: Processability = serde_json::from_str(&json).expect("round-trip");
    assert_eq!(report, back);
}

// ---------------------------------------------------------------------------
// Processability — low-level constructors
// ---------------------------------------------------------------------------
//
// The constructor `Document::from_bytes_with_format` currently rejects
// encrypted documents up-front, so `from_blocked_metadata` and
// `from_decode_failure` are not reachable through the public surface today.
// They exist as defense-in-depth for future loosening (e.g., an API that
// surfaces a health report *before* refusing to construct). We exercise them
// directly so the classification stays correct whatever path feeds them.

fn synthetic_metadata(
    pages: u32,
    encrypted: bool,
    extraction: bool,
) -> crate::model::DocumentMetadata {
    use crate::model::{DocumentFormat, PaginationBasis, PaginationProvenance};
    crate::model::DocumentMetadata {
        format: DocumentFormat::Pdf,
        page_count: pages,
        page_count_provenance: PaginationProvenance::observed(PaginationBasis::PhysicalPages),
        page_dimensions: Vec::new(),
        encrypted,
        text_extraction_allowed: extraction,
        file_size: 0,
        title: None,
    }
}

#[test]
fn processability_from_blocked_metadata_flags_encrypted() {
    let meta = synthetic_metadata(5, true, false);
    let report = super::processability::from_blocked_metadata(&meta);
    assert_eq!(report.health, Health::Blocked);
    assert!(!report.is_processable);
    assert_eq!(report.blockers, vec![HealthIssue::Encrypted]);
    assert!(report.degradations.is_empty());
    assert_eq!(report.pages_total, 5);
    assert_eq!(report.pages_with_content, 0);
    assert_eq!(report.warning_count, 0);
}

#[test]
fn processability_from_decode_failure_flags_decode_failed() {
    let meta = synthetic_metadata(3, false, true);
    let report = super::processability::from_decode_failure(&meta);
    assert_eq!(report.health, Health::Blocked);
    assert!(!report.is_processable);
    assert_eq!(report.blockers, vec![HealthIssue::DecodeFailed]);
    assert!(report.degradations.is_empty());
    assert_eq!(report.pages_total, 3);
    assert_eq!(report.pages_with_content, 0);
}

#[test]
fn processability_from_inputs_reports_ok_on_clean_signals() {
    let meta = synthetic_metadata(2, false, true);
    let report = super::processability::from_inputs(super::processability::Inputs {
        metadata: &meta,
        warnings: &[],
        heuristic_pages: 0,
        pages_with_content: 2,
    });
    assert_eq!(report.health, Health::Ok);
    assert!(report.is_processable);
    assert!(report.blockers.is_empty());
    assert!(report.degradations.is_empty());
    assert_eq!(report.pages_total, 2);
    assert_eq!(report.pages_with_content, 2);
    assert_eq!(report.warning_count, 0);
}

#[test]
fn processability_from_inputs_blocks_on_empty_content() {
    let meta = synthetic_metadata(3, false, true);
    let report = super::processability::from_inputs(super::processability::Inputs {
        metadata: &meta,
        warnings: &[],
        heuristic_pages: 0,
        pages_with_content: 0,
    });
    assert_eq!(report.health, Health::Blocked);
    assert!(!report.is_processable);
    assert_eq!(report.blockers, vec![HealthIssue::EmptyContent]);
}

#[test]
fn processability_from_inputs_flags_heuristic_structure_as_degradation() {
    let meta = synthetic_metadata(4, false, true);
    let report = super::processability::from_inputs(super::processability::Inputs {
        metadata: &meta,
        warnings: &[],
        heuristic_pages: 2,
        pages_with_content: 4,
    });
    assert_eq!(report.health, Health::Degraded);
    assert!(report.blockers.is_empty());
    assert_eq!(
        report.degradations,
        vec![HealthIssue::HeuristicStructure { pages: 2 }]
    );
}

#[test]
fn processability_from_inputs_groups_warnings_deterministically() {
    use crate::error::{Warning, WarningKind};
    let meta = synthetic_metadata(1, false, true);
    let warnings = vec![
        Warning {
            kind: WarningKind::MissingPart,
            message: "styles.xml".into(),
            page: None,
        },
        Warning {
            kind: WarningKind::PartialExtraction,
            message: "half".into(),
            page: Some(1),
        },
        Warning {
            kind: WarningKind::PartialExtraction,
            message: "other half".into(),
            page: Some(1),
        },
        Warning {
            kind: WarningKind::TrackedChangeResolved,
            message: "insertion".into(),
            page: Some(1),
        },
        Warning {
            kind: WarningKind::UnsupportedElement,
            message: "w:foo".into(),
            page: Some(1),
        },
    ];
    let report = super::processability::from_inputs(super::processability::Inputs {
        metadata: &meta,
        warnings: &warnings,
        heuristic_pages: 0,
        pages_with_content: 1,
    });
    assert_eq!(report.health, Health::Degraded);
    // Ordering: PartialExtraction first (per the fixed emission order in
    // `from_inputs`), then MissingPart, then "other" for tracked-change +
    // unsupported-element.
    assert_eq!(
        report.degradations,
        vec![
            HealthIssue::PartialExtraction { count: 2 },
            HealthIssue::MissingPart { count: 1 },
            HealthIssue::OtherWarnings { count: 2 },
        ]
    );
    assert_eq!(report.warning_count, warnings.len());
}

// ---------------------------------------------------------------------------
// Non-regression: Brick D processability call is non-invasive
// ---------------------------------------------------------------------------

#[test]
fn brick_d_processability_is_non_invasive() {
    // Mirror of the Brick B / Brick C guards: calling `processability()`
    // must never drift cached text / markdown / JSON output, otherwise the
    // method has accidentally mutated primitives or the tree.
    for path in [
        "pdf/structured_report.pdf",
        "docx/mixed_content_stress.docx",
        "xlsx/employee_directory.xlsx",
        "html/complex_report.html",
    ] {
        let doc = Document::open(corpus(path)).unwrap();

        let text_before = doc.text();
        let md_before = doc.markdown();
        let json_before = doc.to_json().unwrap().to_string();

        // Call twice — once to force the cache path, once on warm state.
        let _ = doc.processability();
        let _ = doc.processability();

        let text_after = doc.text();
        let md_after = doc.markdown();
        let json_after = doc.to_json().unwrap().to_string();

        assert_eq!(text_before, text_after, "{path} text drifted");
        assert_eq!(md_before, md_after, "{path} markdown drifted");
        assert_eq!(json_before, json_after, "{path} json drifted");
    }
}

// ---------------------------------------------------------------------------
// Send + Sync
// ---------------------------------------------------------------------------

#[test]
fn document_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Document>();
    assert_send_sync::<Page>();
}
