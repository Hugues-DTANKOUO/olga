//! End-to-end integration tests for the `olga` CLI.
//!
//! Each test invokes the compiled `olga` binary via [`assert_cmd`] against a
//! fixture from the corpus, then asserts on the JSON payload or the exit
//! status. These tests cover Brick E — the `inspect` / `search` / `pages`
//! subcommands — and keep the default `olga <file>` processing path unbroken.

use std::path::PathBuf;

use assert_cmd::Command;
use serde_json::Value;

fn corpus(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("corpus")
        .join(path)
}

/// Run `olga` with the given args and parse stdout as JSON. Panics with the
/// full output on failure so diagnostics are visible in CI.
fn run_json(args: &[&str]) -> Value {
    let output = Command::cargo_bin("olga")
        .expect("olga binary must build")
        .args(args)
        .output()
        .expect("olga CLI must exec");
    assert!(
        output.status.success(),
        "olga {args:?} exited with {:?}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout must be UTF-8");
    serde_json::from_str(&stdout).expect("stdout must be JSON")
}

// ---------------------------------------------------------------------------
// inspect
// ---------------------------------------------------------------------------

#[test]
fn inspect_emits_enriched_metadata_with_health_report() {
    let pdf = corpus("pdf/structured_report.pdf");
    let json = run_json(&["inspect", pdf.to_str().unwrap()]);

    // Core metadata fields stay stable.
    assert_eq!(json["format"], "PDF");
    assert!(json["page_count"].as_u64().unwrap() >= 1);
    assert_eq!(json["encrypted"], false);

    // Brick D — processability report is embedded.
    let health = &json["processability"];
    let verdict = health["health"].as_str().unwrap();
    assert!(matches!(verdict, "ok" | "degraded"));
    assert_eq!(health["is_processable"], true);
    assert!(health["blockers"].as_array().unwrap().is_empty());
    assert_eq!(health["pages_total"], json["page_count"]);

    // Content counts are present and numeric.
    let counts = &json["counts"];
    assert!(counts["warnings"].as_u64().is_some());
    assert!(counts["images"].as_u64().is_some());
    assert!(counts["links"].as_u64().is_some());
    assert!(counts["tables"].as_u64().is_some());
    assert!(counts["pages_with_content"].as_u64().is_some());
}

#[test]
fn inspect_for_every_format_reports_health() {
    for path in [
        "pdf/structured_report.pdf",
        "docx/project_status.docx",
        "xlsx/employee_directory.xlsx",
        "html/complex_report.html",
    ] {
        let p = corpus(path);
        let json = run_json(&["inspect", p.to_str().unwrap()]);
        let verdict = json["processability"]["health"].as_str().unwrap();
        assert!(
            matches!(verdict, "ok" | "degraded"),
            "{path} unexpectedly blocked: {:?}",
            json["processability"]["blockers"]
        );
    }
}

// ---------------------------------------------------------------------------
// search
// ---------------------------------------------------------------------------

#[test]
fn search_returns_hits_with_snippets() {
    let pdf = corpus("pdf/structured_report.pdf");
    let json = run_json(&["search", pdf.to_str().unwrap(), "the"]);

    assert_eq!(json["query"], "the");
    assert!(json["page_filter"].is_null());

    let hits = json["hits"].as_array().unwrap();
    assert!(!hits.is_empty(), "expected at least one hit for 'the'");
    for hit in hits {
        // All required fields are present per hit.
        let snippet = hit["snippet"].as_str().unwrap();
        let match_text = hit["match"].as_str().unwrap();
        assert!(
            snippet.to_lowercase().contains(&match_text.to_lowercase()),
            "snippet must contain the match: snippet={snippet:?} match={match_text:?}"
        );
        assert!(hit["page"].as_u64().is_some());
        assert!(hit["line"].as_u64().is_some());
        assert!(hit["col_start"].as_u64().is_some());
    }
}

#[test]
fn search_limit_truncates_hits_but_keeps_total_before_limit() {
    let pdf = corpus("pdf/structured_report.pdf");
    let full = run_json(&["search", pdf.to_str().unwrap(), "the"]);
    let full_count = full["hits"].as_array().unwrap().len();

    if full_count < 2 {
        return; // degenerate fixture — skip rather than fake a trimmed case.
    }

    let limited = run_json(&["search", pdf.to_str().unwrap(), "the", "--limit", "1"]);
    assert_eq!(limited["hits"].as_array().unwrap().len(), 1);
    assert_eq!(
        limited["total_before_limit"].as_u64().unwrap() as usize,
        full_count,
        "total_before_limit must reflect the unclipped count"
    );
}

#[test]
fn search_page_filter_restricts_to_that_page() {
    let pdf = corpus("pdf/structured_report.pdf");
    let json = run_json(&["search", pdf.to_str().unwrap(), "the", "--page", "1"]);
    assert_eq!(json["page_filter"].as_u64().unwrap(), 1);
    for hit in json["hits"].as_array().unwrap() {
        assert_eq!(hit["page"].as_u64().unwrap(), 1);
    }
}

#[test]
fn search_empty_query_returns_no_hits() {
    let pdf = corpus("pdf/structured_report.pdf");
    let json = run_json(&["search", pdf.to_str().unwrap(), ""]);
    assert!(json["hits"].as_array().unwrap().is_empty());
    assert_eq!(json["hit_count"].as_u64().unwrap(), 0);
}

// ---------------------------------------------------------------------------
// pages
// ---------------------------------------------------------------------------

#[test]
fn pages_summary_lists_every_page_with_char_counts() {
    let pdf = corpus("pdf/structured_report.pdf");
    let json = run_json(&["pages", pdf.to_str().unwrap()]);
    let total = json["page_count"].as_u64().unwrap();
    let entries = json["pages"].as_array().unwrap();
    assert_eq!(entries.len() as u64, total);
    for (i, entry) in entries.iter().enumerate() {
        assert_eq!(entry["page"].as_u64().unwrap() as usize, i + 1);
        assert!(entry["char_count"].as_u64().is_some());
        assert!(entry["has_content"].as_bool().is_some());
        assert!(entry["image_count"].as_u64().is_some());
        assert!(entry["link_count"].as_u64().is_some());
        assert!(entry["table_count"].as_u64().is_some());
    }
}

#[test]
fn pages_single_page_emits_rendered_text() {
    let pdf = corpus("pdf/structured_report.pdf");
    let output = Command::cargo_bin("olga")
        .expect("olga binary must build")
        .args(["pages", pdf.to_str().unwrap(), "--page", "1"])
        .output()
        .expect("olga CLI must exec");
    assert!(output.status.success());
    let body = String::from_utf8(output.stdout).expect("utf-8");
    // Not JSON — raw text body on stdout.
    assert!(serde_json::from_str::<Value>(&body).is_err());
    assert!(
        body.ends_with('\n'),
        "trailing newline for pipe-friendliness"
    );
    assert!(!body.trim().is_empty(), "page 1 must have content");
}

#[test]
fn pages_out_of_range_fails_with_nonzero_exit() {
    let pdf = corpus("pdf/structured_report.pdf");
    let output = Command::cargo_bin("olga")
        .expect("olga binary must build")
        .args(["pages", pdf.to_str().unwrap(), "--page", "9999"])
        .output()
        .expect("olga CLI must exec");
    assert!(!output.status.success(), "out-of-range must exit non-zero");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("out of range"),
        "stderr must explain the failure: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// non-regression: default process command still works
// ---------------------------------------------------------------------------

#[test]
fn default_process_command_still_produces_json() {
    let pdf = corpus("pdf/structured_report.pdf");
    let output = Command::cargo_bin("olga")
        .expect("olga binary must build")
        .args([pdf.to_str().unwrap(), "--compact", "--quiet"])
        .output()
        .expect("olga CLI must exec");
    assert!(
        output.status.success(),
        "default `olga <file>` command must still succeed"
    );
    let stdout = String::from_utf8(output.stdout).expect("utf-8");
    let json: Value = serde_json::from_str(&stdout).expect("default output must be JSON");
    // The default payload carries the structure tree, not the Brick E shape.
    assert!(json.is_object() || json.is_array());
}
