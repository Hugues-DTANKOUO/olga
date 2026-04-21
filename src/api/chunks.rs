//! Per-page text chunks suitable for embedding / RAG pipelines.
//!
//! v1 policy: **one chunk per non-empty page**. The text is exactly what
//! [`crate::api::Document::text_by_page`] returns, wrapped in a typed
//! record that also carries the page number. Token-aware splitting, sliding
//! windows, and other chunking strategies are deliberately deferred — they
//! belong in a downstream crate so we don't couple the core engine to a
//! specific tokenizer or target model. Consumers who need finer-grained
//! chunks can post-process these per-page records.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A chunk of text scoped to a single page.
///
/// `page` is **1-based**. `char_count` is the Unicode-scalar count of
/// `text`, precomputed so consumers building batched embeddings can size
/// their batches without re-scanning the string.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chunk {
    pub page: u32,
    pub text: String,
    pub char_count: usize,
}

impl Chunk {
    fn new(page: u32, text: String) -> Self {
        let char_count = text.chars().count();
        Self {
            page,
            text,
            char_count,
        }
    }
}

/// Build a chunk list from the cached text-by-page map.
///
/// Empty pages are skipped — an embedder call with no content is wasted
/// work for all downstream consumers.
pub(crate) fn chunks_by_page(text_by_page: &BTreeMap<usize, String>) -> Vec<Chunk> {
    text_by_page
        .iter()
        .filter(|(_, text)| !text.trim().is_empty())
        .map(|(page, text)| Chunk::new(*page as u32, text.clone()))
        .collect()
}

/// Single-page variant — used by [`crate::api::Page::chunk`].
pub(crate) fn chunk_for_page(page: u32, text: &str) -> Option<Chunk> {
    if text.trim().is_empty() {
        None
    } else {
        Some(Chunk::new(page, text.to_string()))
    }
}
