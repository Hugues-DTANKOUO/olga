//! `Document` — the top-level public handle for a parsed input.
//!
//! A [`Document`] owns the original bytes, pre-flight metadata, and lazy
//! caches of:
//!   * the full [`DecodeResult`] (primitives + warnings);
//!   * a per-page text map;
//!   * a per-page markdown map;
//!   * the structured [`StructureResult`] used by `to_json` and `outline`.
//!
//! Caches use `OnceLock` so initialization is race-safe and work is done at
//! most once per document instance.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use serde_json::Value;

use crate::error::{IdpError, IdpResult, Warning};
use crate::formats::docx::DocxDecoder;
use crate::formats::html::HtmlDecoder;
use crate::formats::pdf::PdfDecoder;
use crate::formats::xlsx::XlsxDecoder;
use crate::model::{DocumentMetadata, ExtractedImage, Primitive};
use crate::output::{self, markdown as md_render, prim_spatial, spatial};
use crate::structure::{StructureConfig, StructureEngine, StructureResult};
use crate::traits::{DecodeResult, FormatDecoder};

use super::chunks::{self, Chunk};
use super::format::{Format, detect_format, format_from_extension};
use super::links::{self, Link};
use super::outline::OutlineEntry;
use super::page::Page;
use super::processability::{self, Processability};
use super::search::{self, SearchHit};
use super::tables::{self, Table};

/// A parsed document, entry point to everything the public API exposes.
///
/// `Document` is `Send + Sync`. It is not `Clone` on purpose — heavy caches
/// live inside an internal `Arc`, so share it via `Arc<Document>` if multiple
/// owners need it.
pub struct Document {
    inner: Arc<DocumentInner>,
}

impl Document {
    // -----------------------------------------------------------------------
    // Constructors
    // -----------------------------------------------------------------------

    /// Open a document from a filesystem path.
    ///
    /// The format is detected first from magic bytes, then falls back to the
    /// file extension if the byte signature is ambiguous.
    pub fn open(path: impl AsRef<Path>) -> IdpResult<Self> {
        let path = path.as_ref();
        let data = fs::read(path)?;

        // Prefer magic-byte detection; fall back to the file extension when
        // sniffing fails (e.g., files without a PDF header that we trust by
        // extension, or ZIP archives whose flavor we can't determine).
        let format = match detect_format(&data) {
            Ok(f) => f,
            Err(sniff_err) => match path.extension().and_then(|s| s.to_str()) {
                Some(ext) => format_from_extension(ext)?,
                None => return Err(sniff_err),
            },
        };

        Self::from_bytes_with_format(data, format)
    }

    /// Open a document from raw bytes already held in memory.
    ///
    /// If `format_hint` is `Some`, it is used directly. Otherwise the format
    /// is inferred from magic bytes via [`detect_format`].
    pub fn open_bytes(data: Vec<u8>, format_hint: Option<Format>) -> IdpResult<Self> {
        let format = match format_hint {
            Some(f) => f,
            None => detect_format(&data)?,
        };
        Self::from_bytes_with_format(data, format)
    }

    fn from_bytes_with_format(data: Vec<u8>, format: Format) -> IdpResult<Self> {
        // Pre-flight: validate metadata so constructors fail loudly on
        // unreadable / encrypted / unsupported inputs.
        let metadata = metadata_for(format, &data)?;

        if !metadata.is_processable() {
            return Err(IdpError::EncryptedDocument);
        }

        Ok(Self {
            inner: Arc::new(DocumentInner {
                format,
                data,
                metadata,
                cached_decode: OnceLock::new(),
                cached_structure: OnceLock::new(),
                cached_text_by_page: OnceLock::new(),
                cached_markdown_by_page: OnceLock::new(),
            }),
        })
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    /// The detected input format.
    pub fn format(&self) -> Format {
        self.inner.format
    }

    /// Number of pages. Always `>= 1` for a processable document.
    ///
    /// Semantics by format:
    /// - PDF: real page count from the document.
    /// - XLSX: sheet count.
    /// - HTML: always 1.
    /// - DOCX: best-effort estimate (see [`DocumentMetadata::page_count`]).
    pub fn page_count(&self) -> usize {
        self.inner.metadata.page_count as usize
    }

    /// Pre-flight metadata (format, pages, size, encryption, title).
    pub fn metadata(&self) -> &DocumentMetadata {
        &self.inner.metadata
    }

    /// Non-fatal warnings collected during decoding.
    ///
    /// Returns an empty slice until the document has been decoded. Triggering
    /// any content accessor (`text`, `markdown`, `outline`, `to_json`, …)
    /// forces the decode and populates warnings.
    pub fn warnings(&self) -> &[Warning] {
        match self.inner.cached_decode.get() {
            Some(dr) => &dr.warnings,
            None => &[],
        }
    }

    /// Whether the document can be processed (not encrypted, or extraction
    /// is allowed). Shortcut for `metadata().is_processable()`.
    ///
    /// For a richer report that also considers decode outcomes (empty
    /// content, decoder failures, structure heuristics, recoverable
    /// warnings) see [`Self::processability`].
    pub fn is_processable(&self) -> bool {
        self.inner.metadata.is_processable()
    }

    // -----------------------------------------------------------------------
    // Pages
    // -----------------------------------------------------------------------

    /// Iterate over the document's pages in order (1-based on the public API).
    pub fn pages(&self) -> impl Iterator<Item = Page> + '_ {
        let inner = Arc::clone(&self.inner);
        (0..self.page_count()).map(move |index| Page {
            inner: Arc::clone(&inner),
            index,
        })
    }

    /// Fetch one page by its 1-based number. `None` if out of range.
    pub fn page(&self, number: usize) -> Option<Page> {
        if number == 0 || number > self.page_count() {
            return None;
        }
        Some(Page {
            inner: Arc::clone(&self.inner),
            index: number - 1,
        })
    }

    // -----------------------------------------------------------------------
    // Text / Markdown
    // -----------------------------------------------------------------------

    /// Full plain-text extraction, pages concatenated with a blank line.
    pub fn text(&self) -> String {
        join_pages(self.inner.text_by_page())
    }

    /// Per-page plain-text extraction. Keys are 1-based page numbers.
    pub fn text_by_page(&self) -> BTreeMap<usize, String> {
        self.inner.text_by_page().clone()
    }

    /// Full markdown extraction, pages concatenated with a blank line.
    pub fn markdown(&self) -> String {
        join_pages(self.inner.markdown_by_page())
    }

    /// Per-page markdown extraction. Keys are 1-based page numbers.
    pub fn markdown_by_page(&self) -> BTreeMap<usize, String> {
        self.inner.markdown_by_page().clone()
    }

    // -----------------------------------------------------------------------
    // Images
    // -----------------------------------------------------------------------

    /// All raster images extracted from the document, in source order.
    ///
    /// Images live on their own channel — they are not injected into the
    /// primitive stream, so [`Self::text`], [`Self::markdown`], and
    /// [`Self::to_json`] stay byte-identical whether the document carries
    /// images or not. Accessing this method forces a decode on first call.
    ///
    /// Semantics by format:
    /// - PDF: images are extracted via `pdf_oxide::extract_images`, with
    ///   alt-text pulled from the tagged-PDF structure tree when available.
    /// - DOCX / HTML: mirrored from every `PrimitiveKind::Image` the decoder
    ///   already emitted.
    /// - XLSX: empty in v1 (sheet-level media is not wired yet).
    /// - Decode failure: empty slice.
    pub fn images(&self) -> &[ExtractedImage] {
        match self.inner.decode() {
            Ok(dr) => &dr.images,
            Err(_) => &[],
        }
    }

    /// Number of images in the document.
    ///
    /// Forces a decode on first call. Returns `0` if decoding fails.
    pub fn image_count(&self) -> usize {
        self.images().len()
    }

    // -----------------------------------------------------------------------
    // Structure
    // -----------------------------------------------------------------------

    /// The document outline (headings, 1-based pages) in reading order.
    pub fn outline(&self) -> IdpResult<Vec<OutlineEntry>> {
        let structure = self.inner.structure()?;
        Ok(OutlineEntry::collect(&structure.root))
    }

    /// Full structured JSON output — same payload the `olga <file>` command
    /// produces.
    pub fn to_json(&self) -> IdpResult<Value> {
        let structure = self.inner.structure()?;
        Ok(output::json::render(structure))
    }

    // -----------------------------------------------------------------------
    // Links
    // -----------------------------------------------------------------------

    /// All hyperlinks in the document, in source order.
    ///
    /// Drawn from the primitive stream's `HintKind::Link` annotations
    /// (PDF `/Annot /Link`, DOCX `<w:hyperlink>`, HTML `<a href>`).
    /// Consecutive same-URL primitives on the same page are coalesced into
    /// one entry so a multi-word visual link surfaces as one [`Link`].
    /// Returns an empty vec on decode failure.
    pub fn links(&self) -> Vec<Link> {
        match self.inner.decode() {
            Ok(dr) => links::collect_links(&dr.primitives),
            Err(_) => Vec::new(),
        }
    }

    /// Number of links in the document.
    pub fn link_count(&self) -> usize {
        self.links().len()
    }

    // -----------------------------------------------------------------------
    // Tables
    // -----------------------------------------------------------------------

    /// All tables in the document, in document order.
    ///
    /// Walks the structure tree, collecting every `NodeKind::Table` with its
    /// flattened cells. A table that crosses page boundaries appears once,
    /// with `first_page` and `last_page` framing its span. Returns an empty
    /// vec on structuring failure.
    pub fn tables(&self) -> Vec<Table> {
        match self.inner.structure() {
            Ok(sr) => tables::collect_tables(&sr.root),
            Err(_) => Vec::new(),
        }
    }

    /// Number of tables in the document.
    pub fn table_count(&self) -> usize {
        self.tables().len()
    }

    // -----------------------------------------------------------------------
    // Search
    // -----------------------------------------------------------------------

    /// Case-insensitive substring search across the rendered plain-text view.
    ///
    /// Hits carry the 1-based page and line, the 0-based character column,
    /// the matched substring (case-preserving), and the full line for
    /// snippet display. Empty queries return no hits.
    pub fn search(&self, query: &str) -> Vec<SearchHit> {
        search::search_all(self.inner.text_by_page(), query)
    }

    // -----------------------------------------------------------------------
    // Chunks
    // -----------------------------------------------------------------------

    /// Per-page text chunks, suitable for embedding / RAG pipelines.
    ///
    /// One [`Chunk`] per non-empty page in 1-based order. v1 keeps the
    /// chunk == page mapping; finer-grained splitting belongs in a
    /// downstream tokenizer-aware crate.
    pub fn chunks_by_page(&self) -> Vec<Chunk> {
        chunks::chunks_by_page(self.inner.text_by_page())
    }

    // -----------------------------------------------------------------------
    // Processability
    // -----------------------------------------------------------------------

    /// Health-check report consolidating every signal the engine emits.
    ///
    /// Unlike [`Self::is_processable`], which only inspects pre-flight
    /// metadata, this method forces a decode + structure pass and folds in:
    ///   * encryption status and extraction rights,
    ///   * decoder failure (treated as a blocker),
    ///   * zero-content documents (blocker even when warnings are benign),
    ///   * approximate pagination and heuristic structure (degradations),
    ///   * every `WarningKind` emitted by decoders and the structure engine.
    ///
    /// The report is deterministic given a document: each issue kind appears
    /// at most once in `blockers` or `degradations`, and ordering is stable
    /// across runs. See [`Processability`] for the full shape.
    pub fn processability(&self) -> Processability {
        // Fast path: metadata already says we can't process this document.
        // No decode attempt, no cache side effects.
        if !self.inner.metadata.is_processable() {
            return processability::from_blocked_metadata(&self.inner.metadata);
        }

        // Force decode + structure so warnings and heuristic signals are
        // available. A structuring failure downgrades us to `Blocked`.
        let sr = match self.inner.structure() {
            Ok(sr) => sr,
            Err(_) => return processability::from_decode_failure(&self.inner.metadata),
        };

        let text_by_page = self.inner.text_by_page();
        let pages_with_content = text_by_page
            .values()
            .filter(|t| !t.trim().is_empty())
            .count() as u32;

        processability::from_inputs(processability::Inputs {
            metadata: &self.inner.metadata,
            warnings: &sr.warnings,
            heuristic_pages: sr.heuristic_pages.len() as u32,
            pages_with_content,
        })
    }
}

impl std::fmt::Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Document")
            .field("format", &self.inner.format)
            .field("page_count", &self.page_count())
            .field("file_size", &self.inner.metadata.file_size)
            .field("title", &self.inner.metadata.title)
            .field("encrypted", &self.inner.metadata.encrypted)
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// Inner state — shared by the Document and all its Pages.
// ---------------------------------------------------------------------------

pub(super) struct DocumentInner {
    pub(super) format: Format,
    pub(super) data: Vec<u8>,
    pub(super) metadata: DocumentMetadata,

    cached_decode: OnceLock<DecodeResult>,
    cached_structure: OnceLock<StructureResult>,
    cached_text_by_page: OnceLock<BTreeMap<usize, String>>,
    cached_markdown_by_page: OnceLock<BTreeMap<usize, String>>,
}

impl DocumentInner {
    /// Decode the document on demand, caching the result.
    fn decode(&self) -> IdpResult<&DecodeResult> {
        if let Some(dr) = self.cached_decode.get() {
            return Ok(dr);
        }
        let dr = decode_for(self.format, self.data.clone())?;
        // Race-safe: a concurrent caller may have populated the slot first.
        // Either set succeeds and we return `dr` via the cache, or it fails
        // silently and we return the already-stored value.
        let _ = self.cached_decode.set(dr);
        Ok(self
            .cached_decode
            .get()
            .expect("decode cache was just populated"))
    }

    fn structure(&self) -> IdpResult<&StructureResult> {
        if let Some(sr) = self.cached_structure.get() {
            return Ok(sr);
        }
        // Structuring consumes the DecodeResult, so we clone the cached one.
        // Cheap relative to the work the structure engine does.
        let dr = self.decode()?;
        let cloned = DecodeResult {
            metadata: dr.metadata.clone(),
            primitives: dr.primitives.clone(),
            warnings: dr.warnings.clone(),
            // The structure engine ignores `images` — they travel on the
            // dedicated Document::images() channel.
            images: Vec::new(),
        };
        let engine = StructureEngine::new(StructureConfig::default()).with_default_detectors();
        let sr = engine.structure(cloned);
        let _ = self.cached_structure.set(sr);
        Ok(self
            .cached_structure
            .get()
            .expect("structure cache was just populated"))
    }

    /// Raster images for a single page (0-based index).
    ///
    /// Returns an empty vec if decoding fails or the page carries no images.
    /// Cheap enough to allocate on each call — images are filtered in place
    /// from the cached `DecodeResult::images`.
    pub(super) fn page_images(&self, index: usize) -> Vec<&ExtractedImage> {
        let Ok(dr) = self.decode() else {
            return Vec::new();
        };
        let page_u32 = index as u32;
        dr.images
            .iter()
            .filter(|img| img.page == page_u32)
            .collect()
    }

    /// Links on a single page (0-based index).
    ///
    /// Walks the primitive stream and applies the same coalescing rule as
    /// [`Document::links`], then filters to the requested page.
    pub(super) fn page_links(&self, index: usize) -> Vec<Link> {
        let Ok(dr) = self.decode() else {
            return Vec::new();
        };
        let page_num = (index as u32).saturating_add(1);
        let all = links::collect_links(&dr.primitives);
        links::filter_by_page(&all, page_num)
    }

    /// Tables that cover a single page (0-based index).
    ///
    /// A table that spans several pages appears on every page it covers.
    pub(super) fn page_tables(&self, index: usize) -> Vec<Table> {
        let Ok(sr) = self.structure() else {
            return Vec::new();
        };
        let page_num = (index as u32).saturating_add(1);
        let all = tables::collect_tables(&sr.root);
        tables::tables_on_page(&all, page_num)
    }

    /// Search a single page (0-based index) for `query`.
    pub(super) fn page_search(&self, index: usize, query: &str) -> Vec<SearchHit> {
        let page_num = (index as u32).saturating_add(1);
        let map = self.text_by_page();
        match map.get(&(page_num as usize)) {
            Some(text) => search::search_one_page(page_num, text, query),
            None => Vec::new(),
        }
    }

    /// Text chunk for a single page (0-based index).
    pub(super) fn page_chunk(&self, index: usize) -> Option<Chunk> {
        let page_num = (index as u32).saturating_add(1);
        let map = self.text_by_page();
        let text = map.get(&(page_num as usize))?;
        chunks::chunk_for_page(page_num, text)
    }

    pub(super) fn text_by_page(&self) -> &BTreeMap<usize, String> {
        if let Some(m) = self.cached_text_by_page.get() {
            return m;
        }
        let map = self.render_by_page(RenderKind::Text);
        let _ = self.cached_text_by_page.set(map);
        self.cached_text_by_page
            .get()
            .expect("text cache was just populated")
    }

    pub(super) fn markdown_by_page(&self) -> &BTreeMap<usize, String> {
        if let Some(m) = self.cached_markdown_by_page.get() {
            return m;
        }
        let map = self.render_by_page(RenderKind::Markdown);
        let _ = self.cached_markdown_by_page.set(map);
        self.cached_markdown_by_page
            .get()
            .expect("markdown cache was just populated")
    }

    fn render_by_page(&self, kind: RenderKind) -> BTreeMap<usize, String> {
        match self.format {
            Format::Pdf => render_pdf_by_page(&self.data, kind),
            Format::Docx | Format::Xlsx => match self.decode() {
                Ok(dr) => render_primitives_by_page(&dr.primitives, &dr.metadata, kind),
                Err(_) => BTreeMap::new(),
            },
            Format::Html | Format::Other(_) => match self.decode() {
                Ok(dr) => render_html_by_page(dr, kind),
                Err(_) => BTreeMap::new(),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
enum RenderKind {
    Text,
    Markdown,
}

fn decoder_for(format: Format) -> IdpResult<Box<dyn FormatDecoder>> {
    match format {
        Format::Pdf => Ok(Box::new(PdfDecoder)),
        Format::Docx => Ok(Box::new(DocxDecoder)),
        Format::Xlsx => Ok(Box::new(XlsxDecoder)),
        Format::Html => Ok(Box::new(HtmlDecoder)),
        Format::Other(id) => Err(IdpError::UnsupportedFormat(format!(
            "unknown format id {id}"
        ))),
    }
}

fn metadata_for(format: Format, data: &[u8]) -> IdpResult<DocumentMetadata> {
    decoder_for(format)?.metadata(data)
}

fn decode_for(format: Format, data: Vec<u8>) -> IdpResult<DecodeResult> {
    decoder_for(format)?.decode(data)
}

/// Render PDF text / markdown per page using the char-level spatial pipeline.
fn render_pdf_by_page(data: &[u8], kind: RenderKind) -> BTreeMap<usize, String> {
    let mut out = BTreeMap::new();
    match kind {
        RenderKind::Text => {
            let pages = spatial::render_from_bytes(data, &spatial::SpatialConfig::default());
            for page in pages {
                // page_number is 0-based inside the spatial renderer.
                let joined = page.lines.join("\n");
                out.insert(page.page_number as usize + 1, joined);
            }
        }
        RenderKind::Markdown => {
            let pages = md_render::render_from_bytes(data, &md_render::MarkdownConfig::default());
            for page in pages {
                let joined = page.lines.join("\n");
                out.insert(page.page_number as usize + 1, joined);
            }
        }
    }
    out
}

/// Render DOCX / XLSX per page by filtering primitives down to one page and
/// calling the primitive-level spatial renderer once per page.
fn render_primitives_by_page(
    primitives: &[Primitive],
    metadata: &DocumentMetadata,
    kind: RenderKind,
) -> BTreeMap<usize, String> {
    let mut out = BTreeMap::new();
    // Iterate the union of `metadata.page_count` and the set of page
    // indices that primitives actually occupy. The DOCX decoder enforces
    // `page_count >= max(p.page) + 1` as an invariant, so in practice the
    // two agree; this union is the defensive case for any future decoder
    // (or bug) that might under-report and would otherwise drop content
    // silently. See the companion comment in `output::prim_spatial`.
    let max_prim_page = primitives.iter().map(|p| p.page).max();
    let page_upper_exclusive = match (metadata.page_count, max_prim_page) {
        (0, None) => 0,
        (n, None) => n,
        (n, Some(m)) => n.max(m + 1),
    };
    for page_idx in 0..page_upper_exclusive {
        // Collect primitives that live on this page and rebase them to page 0
        // so the renderer sees a single-page document.
        let mut page_prims: Vec<Primitive> = primitives
            .iter()
            .filter(|p| p.page == page_idx)
            .cloned()
            .collect();
        if page_prims.is_empty() {
            continue;
        }
        for p in &mut page_prims {
            p.page = 0;
        }

        // Shape a per-page metadata clone so render_impl's `for 0..page_count`
        // emits exactly this page.
        let mut page_meta = metadata.clone();
        page_meta.page_count = 1;
        if !page_meta.page_dimensions.is_empty() {
            let dims = page_meta
                .page_dimensions
                .get(page_idx as usize)
                .copied()
                .unwrap_or(page_meta.page_dimensions[0]);
            page_meta.page_dimensions = vec![dims];
        }

        let rendered = match kind {
            RenderKind::Text => prim_spatial::render_text(&page_prims, &page_meta),
            RenderKind::Markdown => prim_spatial::render_markdown(&page_prims, &page_meta),
        };
        // Trim trailing newline noise introduced by the renderer's final
        // `.push('\n')`; we keep content unambiguous for the map view.
        let trimmed = rendered.trim_end().to_string();
        if !trimmed.is_empty() {
            out.insert(page_idx as usize + 1, trimmed);
        }
    }
    out
}

/// HTML flows as a single logical page. Use the tree-based renderers directly
/// on the cached structure result so JSON/text/markdown stay consistent.
fn render_html_by_page(dr: &DecodeResult, kind: RenderKind) -> BTreeMap<usize, String> {
    // Structure the document through the same engine `structure()` uses, but
    // on a clone — we can't borrow the inner cache from here.
    let cloned = DecodeResult {
        metadata: dr.metadata.clone(),
        primitives: dr.primitives.clone(),
        warnings: dr.warnings.clone(),
        // The structure engine ignores `images` — they travel on the
        // dedicated Document::images() channel.
        images: Vec::new(),
    };
    let engine = StructureEngine::new(StructureConfig::default()).with_default_detectors();
    let sr = engine.structure(cloned);
    let rendered = match kind {
        RenderKind::Text => output::text_tree::render(&sr),
        RenderKind::Markdown => output::md_tree::render(&sr),
    };
    let mut out = BTreeMap::new();
    let trimmed = rendered.trim_end().to_string();
    if !trimmed.is_empty() {
        out.insert(1, trimmed);
    }
    out
}

fn join_pages(by_page: &BTreeMap<usize, String>) -> String {
    let mut out = String::new();
    for (i, (_, content)) in by_page.iter().enumerate() {
        if i > 0 {
            out.push_str("\n\n");
        }
        out.push_str(content);
    }
    out
}
