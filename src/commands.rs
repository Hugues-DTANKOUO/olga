//! CLI command implementations built on top of the public [`olga::api`] surface.
//!
//! Each handler returns a JSON value (or, for `pages --page N`, the rendered
//! page text). `main.rs` is responsible for printing and exit-code handling.
//!
//! These commands intentionally route through the public [`Document`] type
//! rather than reaching into decoders directly. That keeps the CLI honest:
//! anything it can show, a library consumer can compute too.

use std::path::Path;

use serde_json::{Value, json};

use olga::api::{Document, Health, HealthIssue, SearchHit};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Open a [`Document`] from the given filesystem path, surfacing any error
/// with the offending path baked into the message.
fn open(path: &Path) -> Result<Document, Box<dyn std::error::Error>> {
    Document::open(path).map_err(|e| -> Box<dyn std::error::Error> {
        format!("cannot open '{}': {}", path.display(), e).into()
    })
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

// ---------------------------------------------------------------------------
// `olga inspect`
// ---------------------------------------------------------------------------

/// Build the enriched JSON payload returned by `olga inspect <file>`.
///
/// Combines pre-flight metadata with a forced decode + structure pass so
/// downstream consumers see one document-level health snapshot. The content
/// counts come from the same APIs that library users would call.
pub fn inspect(input: &Path) -> Result<Value, Box<dyn std::error::Error>> {
    let doc = open(input)?;

    let meta = doc.metadata();
    let report = doc.processability();

    let dimensions: Vec<Value> = meta
        .page_dimensions
        .iter()
        .enumerate()
        .map(|(i, d)| {
            json!({
                "page": i + 1,
                "width_pt": d.effective_width,
                "height_pt": d.effective_height,
                "rotation": d.rotation,
            })
        })
        .collect();

    Ok(json!({
        "filename": input.file_name().unwrap_or_default().to_string_lossy(),
        "format": meta.format.to_string(),
        "page_count": meta.page_count,
        "file_size": meta.file_size,
        "file_size_display": format_size(meta.file_size),
        "encrypted": meta.encrypted,
        "text_extraction_allowed": meta.text_extraction_allowed,
        "title": meta.title,
        "page_dimensions": dimensions,
        "processability": health_to_json(&report),
        "counts": {
            "warnings": report.warning_count,
            "images": doc.image_count(),
            "links": doc.link_count(),
            "tables": doc.table_count(),
            "pages_with_content": report.pages_with_content,
        },
    }))
}

/// Serialize a [`Processability`] report for the CLI. Uses a hand-rolled
/// shape rather than the derived one so the JSON keys stay stable even if
/// we evolve the public type's internals (variant renames, extra fields).
fn health_to_json(report: &olga::api::Processability) -> Value {
    let health = match report.health {
        Health::Ok => "ok",
        Health::Degraded => "degraded",
        Health::Blocked => "blocked",
    };
    json!({
        "health": health,
        "is_processable": report.is_processable,
        "blockers": report.blockers.iter().map(issue_to_json).collect::<Vec<_>>(),
        "degradations": report.degradations.iter().map(issue_to_json).collect::<Vec<_>>(),
        "pages_total": report.pages_total,
        "pages_with_content": report.pages_with_content,
        "warning_count": report.warning_count,
    })
}

/// Serialize one [`HealthIssue`] into a `{kind, payload}` envelope so a
/// consumer can dispatch on the kind without parsing free-form strings.
fn issue_to_json(issue: &HealthIssue) -> Value {
    match issue {
        HealthIssue::Encrypted => json!({"kind": "Encrypted"}),
        HealthIssue::EmptyContent => json!({"kind": "EmptyContent"}),
        HealthIssue::DecodeFailed => json!({"kind": "DecodeFailed"}),
        HealthIssue::ApproximatePagination { basis } => {
            json!({"kind": "ApproximatePagination", "basis": basis.to_string()})
        }
        HealthIssue::HeuristicStructure { pages } => {
            json!({"kind": "HeuristicStructure", "pages": pages})
        }
        HealthIssue::PartialExtraction { count } => {
            json!({"kind": "PartialExtraction", "count": count})
        }
        HealthIssue::MissingPart { count } => json!({"kind": "MissingPart", "count": count}),
        HealthIssue::UnresolvedStyle { count } => {
            json!({"kind": "UnresolvedStyle", "count": count})
        }
        HealthIssue::UnresolvedRelationship { count } => {
            json!({"kind": "UnresolvedRelationship", "count": count})
        }
        HealthIssue::MissingMedia { count } => json!({"kind": "MissingMedia", "count": count}),
        HealthIssue::TruncatedContent { count } => {
            json!({"kind": "TruncatedContent", "count": count})
        }
        HealthIssue::MalformedContent { count } => {
            json!({"kind": "MalformedContent", "count": count})
        }
        HealthIssue::FilteredArtifact { count } => {
            json!({"kind": "FilteredArtifact", "count": count})
        }
        HealthIssue::SuspectedArtifact { count } => {
            json!({"kind": "SuspectedArtifact", "count": count})
        }
        HealthIssue::OtherWarnings { count } => json!({"kind": "OtherWarnings", "count": count}),
    }
}

// ---------------------------------------------------------------------------
// `olga search`
// ---------------------------------------------------------------------------

/// Build the JSON payload returned by `olga search <file> <query>`.
///
/// Behaviour:
/// - Empty `query` → `{ hits: [], hit_count: 0 }` (no surprises, same semantics
///   as the library's [`Document::search`]).
/// - `page` filters the hits to a single 1-based page; a page out of range
///   returns no hits.
/// - `limit` truncates after assembling all hits, so the snippet count is
///   stable across calls.
pub fn search(
    input: &Path,
    query: &str,
    page: Option<usize>,
    limit: Option<usize>,
) -> Result<Value, Box<dyn std::error::Error>> {
    let doc = open(input)?;

    let mut hits = match page {
        Some(n) => match doc.page(n) {
            Some(p) => p.search(query),
            None => Vec::new(),
        },
        None => doc.search(query),
    };

    let total = hits.len();
    if let Some(cap) = limit {
        hits.truncate(cap);
    }

    Ok(json!({
        "query": query,
        "page_filter": page,
        "hits": hits.iter().map(hit_to_json).collect::<Vec<_>>(),
        "hit_count": hits.len(),
        "total_before_limit": total,
    }))
}

fn hit_to_json(hit: &SearchHit) -> Value {
    json!({
        "page": hit.page,
        "line": hit.line,
        "col_start": hit.col_start,
        "match": hit.match_text,
        "snippet": hit.snippet,
    })
}

// ---------------------------------------------------------------------------
// `olga pages`
// ---------------------------------------------------------------------------

/// Result of a `pages` invocation: either the JSON summary, or the raw
/// rendered text for a single page.
pub enum PagesOutput {
    /// JSON summary of every page (number, char count, content flags).
    Summary(Value),
    /// Single-page text or markdown extraction.
    PageText(String),
}

/// Run `olga pages <file> [--page N]`.
///
/// Without `--page`, returns a summary suitable for navigation: per page, the
/// character count and whether the page carries images / links / tables.
/// With `--page N`, returns the rendered text (or markdown when `markdown`
/// is `true`) for that single page so callers can pipe it elsewhere.
pub fn pages(
    input: &Path,
    page: Option<usize>,
    markdown: bool,
) -> Result<PagesOutput, Box<dyn std::error::Error>> {
    let doc = open(input)?;

    if let Some(n) = page {
        let p = doc.page(n).ok_or_else(|| -> Box<dyn std::error::Error> {
            format!(
                "page {} out of range (document has {} page{})",
                n,
                doc.page_count(),
                if doc.page_count() == 1 { "" } else { "s" }
            )
            .into()
        })?;
        let body = if markdown { p.markdown() } else { p.text() };
        return Ok(PagesOutput::PageText(body));
    }

    // Summary mode. Walk the document once, collect per-page stats from the
    // already-cached views to stay non-invasive.
    let text_by_page = doc.text_by_page();
    let entries: Vec<Value> = doc
        .pages()
        .map(|p| {
            let n = p.number();
            let text = text_by_page.get(&n).cloned().unwrap_or_default();
            let dims = p.dimensions().map(|d| {
                json!({
                    "width_pt": d.effective_width,
                    "height_pt": d.effective_height,
                    "rotation": d.rotation,
                })
            });
            json!({
                "page": n,
                "char_count": text.chars().count(),
                "has_content": !text.trim().is_empty(),
                "image_count": p.image_count(),
                "link_count": p.link_count(),
                "table_count": p.table_count(),
                "dimensions": dims,
            })
        })
        .collect();

    Ok(PagesOutput::Summary(json!({
        "filename": input.file_name().unwrap_or_default().to_string_lossy(),
        "page_count": doc.page_count(),
        "pages": entries,
    })))
}
