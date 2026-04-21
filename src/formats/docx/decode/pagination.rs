use crate::model::{PaginationBasis, PaginationProvenance};

use super::DocumentPageCounts;

/// Resolve the logical page count for a DOCX from three independent signals.
///
/// The three signals and what they actually mean:
///
/// * `doc_counts.rendered_pages` — count derived from `w:lastRenderedPageBreak`
///   markers that Word writes at the end of each rendered page during its last
///   layout pass. Most faithful when present, but only written by Word itself
///   (not by `docx` / `python-docx` / `mammoth` / programmatic authors) and
///   stale after edits that didn't trigger a re-layout.
///
/// * `doc_counts.explicit_pages` — number of `<w:br w:type="page"/>` hard
///   breaks plus one. A *structural lower bound*: per ECMA-376, a hard break
///   forces a new page, so the document must have at least this many pages.
///   Cannot over-report.
///
/// * `app_pages` — `<Pages>` value in `docProps/app.xml`. Cached by the
///   authoring app at save time. A *hint*, not ground truth: Word on macOS and
///   programmatic authors frequently write `<Pages>1</Pages>` on multi-page
///   documents. Cannot be trusted to set the ceiling.
///
/// The rule, given that property:
///
/// > Each signal can only *miss* pages, never hallucinate them. Therefore the
/// > page count is the maximum across the signals we actually have.
///
/// Provenance is attributed to the highest-quality source that equals the max,
/// preferring rendered markers > explicit breaks > app metadata > fallback.
/// This matches what a diligent human reviewer would conclude if they were
/// handed the three numbers and asked "how many pages does this file have?"
///
/// See the 2026-04-19 vendor evaluation on memo.docx for the failure mode the
/// priority-based predecessor of this function produced (app.xml's
/// `<Pages>1</Pages>` beat a body with an explicit `w:br w:type="page"`,
/// collapsing page count to 1 and silently dropping half the content
/// downstream).
pub(super) fn resolve_docx_page_count(
    app_pages: Option<u32>,
    doc_counts: DocumentPageCounts,
) -> (u32, PaginationProvenance) {
    let rendered = doc_counts.rendered_pages.unwrap_or(0);
    let explicit = doc_counts.explicit_pages.unwrap_or(0);
    let app = app_pages.unwrap_or(0);

    let max_val = rendered.max(explicit).max(app);

    if max_val == 0 {
        return (
            1,
            PaginationProvenance::estimated(PaginationBasis::FallbackDefault),
        );
    }

    let basis = if doc_counts.rendered_pages == Some(max_val) {
        PaginationBasis::RenderedPageBreakMarkers
    } else if doc_counts.explicit_pages == Some(max_val) {
        PaginationBasis::ExplicitBreaks
    } else {
        PaginationBasis::ApplicationMetadata
    };

    (max_val, PaginationProvenance::estimated(basis))
}

pub(super) fn docx_pagination_warning(
    page_count: u32,
    provenance: PaginationProvenance,
    app_pages: Option<u32>,
    doc_counts: DocumentPageCounts,
) -> Option<String> {
    match provenance.basis {
        PaginationBasis::RenderedPageBreakMarkers => {
            if let Some(app_pages) = app_pages.filter(|app_pages| *app_pages != page_count) {
                Some(format!(
                    "DOCX page count uses w:lastRenderedPageBreak markers ({}) while docProps/app.xml reports {}",
                    page_count, app_pages
                ))
            } else {
                Some(format!(
                    "DOCX page count uses w:lastRenderedPageBreak markers ({}) rather than true rendered pagination",
                    page_count
                ))
            }
        }
        PaginationBasis::ApplicationMetadata => Some(format!(
            "DOCX page count uses docProps/app.xml estimate ({}) rather than rendered pagination",
            page_count
        )),
        PaginationBasis::ExplicitBreaks => Some(format!(
            "DOCX page count ({}) was inferred from explicit page breaks only",
            page_count
        )),
        PaginationBasis::FallbackDefault => {
            if app_pages.is_none()
                && doc_counts.rendered_pages.is_none()
                && doc_counts.explicit_pages.is_none()
            {
                None
            } else {
                Some(format!(
                    "DOCX page count fell back to {} despite incomplete pagination signals",
                    page_count
                ))
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(rendered: Option<u32>, explicit: Option<u32>) -> DocumentPageCounts {
        DocumentPageCounts {
            rendered_pages: rendered,
            explicit_pages: explicit,
        }
    }

    /// No signals at all: fall back to 1 page, flagged as fallback.
    #[test]
    fn no_signals_falls_back_to_one() {
        let (n, prov) = resolve_docx_page_count(None, counts(None, None));
        assert_eq!(n, 1);
        assert_eq!(prov.basis, PaginationBasis::FallbackDefault);
    }

    /// The memo.docx failure mode: app.xml says 1, body has one hard break
    /// (so `explicit_pages = 2`). The max wins and explicit breaks are
    /// credited — this is the regression that was silently dropping half
    /// the content downstream before the refactor.
    #[test]
    fn explicit_breaks_beat_under_reporting_app_xml() {
        let (n, prov) = resolve_docx_page_count(Some(1), counts(None, Some(2)));
        assert_eq!(n, 2);
        assert_eq!(prov.basis, PaginationBasis::ExplicitBreaks);
    }

    /// Rendered markers are the most faithful signal when present and
    /// equal the max: Word itself wrote them during last layout.
    #[test]
    fn rendered_markers_win_provenance_when_they_equal_the_max() {
        let (n, prov) = resolve_docx_page_count(Some(3), counts(Some(3), Some(3)));
        assert_eq!(n, 3);
        assert_eq!(prov.basis, PaginationBasis::RenderedPageBreakMarkers);
    }

    /// App.xml can over-report when there are auto-flow pages that neither
    /// rendered markers nor explicit breaks captured (e.g. decoder skipped
    /// rendered markers, or the body is a single long column with no hard
    /// breaks). Use its value, credit it as such.
    #[test]
    fn app_xml_wins_when_higher_than_body_signals() {
        let (n, prov) = resolve_docx_page_count(Some(5), counts(None, Some(2)));
        assert_eq!(n, 5);
        assert_eq!(prov.basis, PaginationBasis::ApplicationMetadata);
    }

    /// Explicit breaks can exceed rendered markers when the document was
    /// edited after Word's last layout pass but before save (markers are
    /// stale, hard breaks are current). Explicit wins because its count is
    /// a structural invariant.
    #[test]
    fn explicit_beats_stale_rendered_markers() {
        let (n, prov) = resolve_docx_page_count(None, counts(Some(2), Some(4)));
        assert_eq!(n, 4);
        assert_eq!(prov.basis, PaginationBasis::ExplicitBreaks);
    }

    /// Only one signal present: use it, credit it.
    #[test]
    fn single_signal_rendered_only() {
        let (n, prov) = resolve_docx_page_count(None, counts(Some(7), None));
        assert_eq!(n, 7);
        assert_eq!(prov.basis, PaginationBasis::RenderedPageBreakMarkers);
    }

    #[test]
    fn single_signal_app_only() {
        let (n, prov) = resolve_docx_page_count(Some(4), counts(None, None));
        assert_eq!(n, 4);
        assert_eq!(prov.basis, PaginationBasis::ApplicationMetadata);
    }
}
