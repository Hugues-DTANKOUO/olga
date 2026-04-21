//! `Page` — a 1-based handle onto one page of a [`Document`].
//!
//! Pages are cheap wrappers around an `Arc<DocumentInner>` plus a 0-based
//! index. They defer to the document's caches for text/markdown content.

use std::sync::Arc;

use crate::model::{ExtractedImage, PageDimensions};

use super::chunks::Chunk;
use super::document::DocumentInner;
use super::links::Link;
use super::search::SearchHit;
use super::tables::Table;

/// A handle onto a single page of a document.
///
/// Pages are 1-based on the public API: `page.number()` is `1` for the first
/// page. For formats with no physical pagination (HTML), a document always
/// exposes exactly one page.
#[derive(Clone)]
pub struct Page {
    pub(super) inner: Arc<DocumentInner>,
    /// 0-based index into [`DocumentInner`]'s page-keyed structures.
    pub(super) index: usize,
}

impl Page {
    /// 1-based page number on the public API.
    pub fn number(&self) -> usize {
        self.index + 1
    }

    /// Physical dimensions of the page, when the format exposes them.
    ///
    /// Returns `None` for formats (DOCX, XLSX, HTML) whose pagination is
    /// logical rather than rendered — only PDF reliably populates this.
    pub fn dimensions(&self) -> Option<&PageDimensions> {
        self.inner.metadata.page_dimensions.get(self.index)
    }

    /// Plain text content for this page.
    ///
    /// Uses the spatial renderer for PDF (character-level geometry) and the
    /// primitive-level renderer for DOCX/XLSX/HTML.
    pub fn text(&self) -> String {
        self.inner
            .text_by_page()
            .get(&self.number())
            .cloned()
            .unwrap_or_default()
    }

    /// Markdown content for this page (same content as [`Self::text`], plus
    /// inline formatting markers).
    pub fn markdown(&self) -> String {
        self.inner
            .markdown_by_page()
            .get(&self.number())
            .cloned()
            .unwrap_or_default()
    }

    /// Raster images that live on this page.
    ///
    /// Images travel on the [`crate::api::Document::images`] channel rather
    /// than the primitive stream, so this is a cheap per-page filter on the
    /// cached decode. Returns an empty vec if the page has no images or the
    /// document failed to decode.
    pub fn images(&self) -> Vec<&ExtractedImage> {
        self.inner.page_images(self.index)
    }

    /// Number of images on this page.
    pub fn image_count(&self) -> usize {
        self.images().len()
    }

    /// Hyperlinks on this page, in source order.
    ///
    /// See [`crate::api::Document::links`] for the coalescing semantics.
    pub fn links(&self) -> Vec<Link> {
        self.inner.page_links(self.index)
    }

    /// Number of hyperlinks on this page.
    pub fn link_count(&self) -> usize {
        self.links().len()
    }

    /// Tables that cover this page.
    ///
    /// A cross-page table appears in the list of every page it spans.
    pub fn tables(&self) -> Vec<Table> {
        self.inner.page_tables(self.index)
    }

    /// Number of tables that cover this page.
    pub fn table_count(&self) -> usize {
        self.tables().len()
    }

    /// Case-insensitive substring search scoped to this page's rendered text.
    ///
    /// See [`crate::api::Document::search`] for the matching semantics.
    pub fn search(&self, query: &str) -> Vec<SearchHit> {
        self.inner.page_search(self.index, query)
    }

    /// The text chunk for this page.
    ///
    /// Returns `None` when the page is empty (or whitespace-only).
    pub fn chunk(&self) -> Option<Chunk> {
        self.inner.page_chunk(self.index)
    }
}

impl std::fmt::Debug for Page {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Page")
            .field("number", &self.number())
            .field("dimensions", &self.dimensions())
            .finish()
    }
}
