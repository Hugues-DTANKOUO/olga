//! `Link` — hyperlinks extracted from the document.
//!
//! Hyperlinks flow through the pipeline as `HintKind::Link { url }` semantic
//! hints attached to text primitives. All three rich formats converge on this
//! representation:
//!
//! - **PDF**: page-level `/Annot /Link` annotations are projected onto the
//!   primitives they cover (see [`crate::formats::pdf::annotations`]).
//! - **DOCX**: `<w:hyperlink>` wrappers tag their inner text runs.
//! - **HTML**: `<a href>` elements tag their inner text nodes.
//!
//! The public API walks the decoded primitive stream, picks out the ones
//! carrying a `Link` hint, and collapses consecutive same-URL runs on the
//! same page into a single [`Link`] entry.
//!
//! # Why consecutive-run coalescing?
//!
//! A single visual hyperlink is almost always split across several text
//! primitives (one per word, one per styled run). Producing one [`Link`]
//! per primitive would yield 3–5 near-duplicate entries per visual link —
//! noisy for search / indexing consumers. We merge primitives that satisfy
//! all three rules: same page, identical URL, and immediately adjacent
//! source-order. The result is one entry per visual link in the common
//! case, with a safe fallback: two unrelated links to the same URL that
//! happen to be interleaved with non-link content stay separate.

use serde::{Deserialize, Serialize};

use crate::model::{BoundingBox, HintKind, Primitive, PrimitiveKind};

/// A hyperlink discovered in the document.
///
/// `page` is **1-based** to match the rest of the public API. `bbox` is the
/// normalized bounding box spanning every primitive that contributed to the
/// link (see the module docs for the coalescing rules).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Link {
    /// Anchor text (concatenation of every coalesced primitive's content).
    pub text: String,
    /// Target URL, taken verbatim from the `HintKind::Link` hint.
    pub url: String,
    /// 1-based page number where the link appears.
    pub page: u32,
    /// Bounding box covering every coalesced primitive.
    pub bbox: BoundingBox,
}

/// Collect every link in the document, in source order.
///
/// Primitives are scanned in the order decoders emitted them. Consecutive
/// runs sharing a page + URL are coalesced.
pub(crate) fn collect_links(primitives: &[Primitive]) -> Vec<Link> {
    let mut out: Vec<Link> = Vec::new();
    let mut expected_next_source: Option<u64> = None;

    for prim in primitives {
        // Only text primitives carry link hints, and only ones with a URL count.
        let content = match &prim.kind {
            PrimitiveKind::Text { content, .. } if !content.is_empty() => content,
            _ => {
                expected_next_source = None;
                continue;
            }
        };
        let url = prim.hints.iter().find_map(|h| match &h.kind {
            HintKind::Link { url } if !url.is_empty() => Some(url.clone()),
            _ => None,
        });
        let Some(url) = url else {
            expected_next_source = None;
            continue;
        };

        // 1-based page on the public API.
        let page = prim.page.saturating_add(1);

        // Coalesce when the prior link is on the same page, shares the URL,
        // and the current primitive's source_order is exactly the expected
        // successor (i.e., the text run is contiguous in the decoder output).
        let coalesce = matches!(
            (out.last(), expected_next_source),
            (Some(last), Some(expected))
                if last.page == page
                    && last.url == url
                    && prim.source_order == expected
        );

        if coalesce {
            let last = out.last_mut().expect("checked above");
            last.text.push_str(content);
            last.bbox = last.bbox.merge(&prim.bbox);
        } else {
            out.push(Link {
                text: content.clone(),
                url,
                page,
                bbox: prim.bbox,
            });
        }
        expected_next_source = Some(prim.source_order.saturating_add(1));
    }

    out
}

/// Filter a document-level link list down to a single 1-based page.
pub(crate) fn filter_by_page(links: &[Link], page: u32) -> Vec<Link> {
    links.iter().filter(|l| l.page == page).cloned().collect()
}
