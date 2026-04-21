//! Text extraction for PDF pages: raw-char grouping, line clustering,
//! direction inference, document-language detection, and the layout-span
//! fallback.
//!
//! Public surface (re-exported to the parent `pages` module):
//! - [`group_chars_into_spans`] — the primary `extract_chars` pipeline
//! - [`reorder_spans_by_lines`] — line-based reordering for the `extract_spans` fallback
//! - [`layout_spans_to_text_spans`] — convert pdf_oxide layout spans to internal `TextSpan`s
//! - [`infer_document_language_tag`] — document-level language inference from page-zero chars
//!
//! Internal layout:
//! - [`clustering`] — skew-aware line clustering over raw chars
//! - [`direction`] — text-direction inference and direction-dependent geometry
//! - [`grouping`] — `group_chars_into_spans` + `flush_span`
//! - [`language`] — CJK + RTL script counting for `/Lang` tag inference
//! - [`layout_path`] — `layout_spans_to_text_spans` (extract_spans fallback)
//! - [`reorder`] — post-grouping line reordering

mod clustering;
mod direction;
mod grouping;
mod language;
mod layout_path;
mod reorder;

pub(crate) use grouping::group_chars_into_spans;
pub(crate) use language::infer_document_language_tag;
pub(crate) use layout_path::layout_spans_to_text_spans;
pub(crate) use reorder::reorder_spans_by_lines;
