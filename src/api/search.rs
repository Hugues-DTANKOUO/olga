//! Text search across the rendered plain-text view.
//!
//! Search runs against the cached `text_by_page` output — the same content
//! [`crate::api::Document::text`] and [`crate::api::Page::text`] return. A
//! query is matched as a case-insensitive **substring** (ASCII-lowercased).
//! That's enough for the common "find all mentions of X" workflow without
//! pulling in a full regex engine or Unicode case-folding dependency.
//!
//! Each [`SearchHit`] carries the page, line number, character offset, the
//! matched substring and its full line for context. Both page and line are
//! **1-based** to match the rest of the public API. Character offsets
//! (`col_start`) are 0-based Unicode-scalar positions within the line.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A single match returned by [`crate::api::Document::search`] or
/// [`crate::api::Page::search`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchHit {
    /// 1-based page number.
    pub page: u32,
    /// 1-based line number within the page's rendered text.
    pub line: u32,
    /// 0-based character column where the match starts.
    pub col_start: u32,
    /// The substring that matched — preserves original casing.
    pub match_text: String,
    /// The full line the match was found on, for UI context / snippets.
    pub snippet: String,
}

/// Search every page in `text_by_page` for `query` (case-insensitive).
///
/// Empty queries yield an empty hit list — otherwise every single character
/// on every page would match.
pub(crate) fn search_all(text_by_page: &BTreeMap<usize, String>, query: &str) -> Vec<SearchHit> {
    if query.is_empty() {
        return Vec::new();
    }
    let needle = query.to_ascii_lowercase();
    let mut out = Vec::new();
    for (page, text) in text_by_page {
        search_page(*page as u32, text, &needle, &mut out);
    }
    out
}

/// Search a single page's text. `page` is already 1-based on the public API.
pub(crate) fn search_one_page(page: u32, text: &str, query: &str) -> Vec<SearchHit> {
    if query.is_empty() {
        return Vec::new();
    }
    let needle = query.to_ascii_lowercase();
    let mut out = Vec::new();
    search_page(page, text, &needle, &mut out);
    out
}

fn search_page(page: u32, text: &str, needle_lower: &str, out: &mut Vec<SearchHit>) {
    // Iterate line-by-line so `line` and `col_start` are naturally scoped
    // to what the user sees in the plain-text rendering.
    for (line_idx, line) in text.lines().enumerate() {
        let line_lower = line.to_ascii_lowercase();
        // `find` returns BYTE indices. We need character-column offsets, so
        // we convert byte offsets to char counts with `chars().take()`-style
        // accounting per hit.
        let mut from = 0usize;
        while from <= line_lower.len() {
            let Some(byte_pos) = line_lower[from..].find(needle_lower) else {
                break;
            };
            let abs_byte = from + byte_pos;
            // Characters up to the match in the *original* line.
            let col_start = line[..abs_byte].chars().count() as u32;
            // Slice the original text (preserving case) using the byte span
            // of the lowercased needle — the two strings have the same byte
            // length because ASCII-lowercasing is per-byte here. Non-ASCII
            // queries still work for the substring boundary: we just emit
            // the original (case-preserving) slice.
            let end_byte = abs_byte + needle_lower.len();
            // Guard against non-ASCII producing an unexpected boundary.
            // ASCII-lowercasing is 1:1 byte-wise, so this slice is safe for
            // ASCII queries. For non-ASCII we re-extract by char count.
            let match_text = if line.is_char_boundary(abs_byte) && line.is_char_boundary(end_byte) {
                line[abs_byte..end_byte].to_string()
            } else {
                // Fallback: take `needle` char count from `col_start`.
                line.chars()
                    .skip(col_start as usize)
                    .take(needle_lower.chars().count())
                    .collect()
            };

            out.push(SearchHit {
                page,
                line: (line_idx as u32).saturating_add(1),
                col_start,
                match_text,
                snippet: line.to_string(),
            });

            // Advance by at least one byte past the start to find overlapping
            // matches on pathological inputs ("aaa" searching for "aa" → two
            // hits). We still use a byte advance for simplicity.
            from = abs_byte + 1;
        }
    }
}
