//! Public format alias and byte-based format detection.
//!
//! The public API re-exposes the internal [`DocumentFormat`] enum under the
//! friendlier name `Format`. A [`detect_format`] helper sniffs the input's
//! first bytes so callers can pass raw bytes without needing a file extension.
//!
//! Detection is intentionally narrow and fast — we look at magic bytes plus,
//! for ZIP-based OOXML files, a small scan of the ZIP central directory to
//! tell DOCX apart from XLSX.

use std::io::Cursor;

use crate::error::{IdpError, IdpResult};
use crate::model::DocumentFormat;

/// Public alias for the internal [`DocumentFormat`] enum.
///
/// Exposed under `api::Format` so the import path on the public API stays
/// ergonomic (`use olga::api::Format;`).
pub type Format = DocumentFormat;

/// Detect the format of a byte slice using magic-byte sniffing.
///
/// Supported signatures:
/// - `%PDF-` → [`Format::Pdf`]
/// - `PK\x03\x04` (ZIP) with a `word/` entry → [`Format::Docx`]
/// - `PK\x03\x04` (ZIP) with an `xl/` entry → [`Format::Xlsx`]
/// - first kilobytes contain `<!doctype html` or `<html` (case-insensitive)
///   → [`Format::Html`]
///
/// Returns [`IdpError::UnsupportedFormat`] if no signature matches.
pub fn detect_format(data: &[u8]) -> IdpResult<Format> {
    // --- PDF ---
    if data.starts_with(b"%PDF-") {
        return Ok(Format::Pdf);
    }

    // --- ZIP-based OOXML (DOCX, XLSX) ---
    if data.starts_with(b"PK\x03\x04") {
        return detect_ooxml_flavor(data);
    }

    // --- HTML heuristic on a preview prefix (case-insensitive, byte-level). ---
    if looks_like_html(data) {
        return Ok(Format::Html);
    }

    Err(IdpError::UnsupportedFormat(
        "could not detect format from byte signature (expected PDF, DOCX, XLSX, or HTML)"
            .to_string(),
    ))
}

/// Distinguish DOCX from XLSX by peeking at ZIP entries.
///
/// Both formats share the ZIP + Open Packaging Conventions base, so we rely on
/// the canonical top-level directory: `word/` for DOCX, `xl/` for XLSX.
fn detect_ooxml_flavor(data: &[u8]) -> IdpResult<Format> {
    let mut archive = zip::ZipArchive::new(Cursor::new(data)).map_err(|e| {
        IdpError::UnsupportedFormat(format!("ZIP signature present but archive is invalid: {e}"))
    })?;

    let mut has_word = false;
    let mut has_xl = false;

    for i in 0..archive.len() {
        // by_index_raw skips decompression — we only need the name.
        let Ok(file) = archive.by_index_raw(i) else {
            continue;
        };
        let name = file.name();
        if name.starts_with("word/") {
            has_word = true;
            break;
        }
        if name.starts_with("xl/") {
            has_xl = true;
            break;
        }
    }

    match (has_word, has_xl) {
        (true, _) => Ok(Format::Docx),
        (_, true) => Ok(Format::Xlsx),
        _ => Err(IdpError::UnsupportedFormat(
            "ZIP archive present but neither DOCX (word/) nor XLSX (xl/) layout detected"
                .to_string(),
        )),
    }
}

/// Cheap HTML detector: look for `<html` or `<!doctype html` in the first 2 KiB.
fn looks_like_html(data: &[u8]) -> bool {
    let preview = &data[..data.len().min(2048)];
    // Manual case-insensitive byte scan — cheaper than constructing a String.
    let lower: Vec<u8> = preview.iter().map(|b| b.to_ascii_lowercase()).collect();
    windows_contains(&lower, b"<!doctype html") || windows_contains(&lower, b"<html")
}

fn windows_contains(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.len() > haystack.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// Detect a format from a filesystem extension string (e.g., "pdf", "docx").
///
/// Accepts common aliases: `docm` → DOCX, `xls` → XLSX, `htm` → HTML.
pub(crate) fn format_from_extension(ext: &str) -> IdpResult<Format> {
    match ext.to_lowercase().as_str() {
        "pdf" => Ok(Format::Pdf),
        "docx" | "docm" => Ok(Format::Docx),
        "xlsx" | "xls" => Ok(Format::Xlsx),
        "html" | "htm" => Ok(Format::Html),
        other => Err(IdpError::UnsupportedFormat(format!(
            "unknown extension '.{other}' (supported: pdf, docx, xlsx, html)"
        ))),
    }
}
