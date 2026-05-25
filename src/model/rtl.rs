//! Unicode RTL (right-to-left) script detection.
//!
//! Lightweight standalone detector that all format decoders can use
//! without pulling in a format-specific dep. Previously the HTML
//! decoder reached into `pdf_oxide::text::rtl_detector::is_rtl_text`
//! which made `pdf_oxide` an implicit transitive dependency of `html`
//! — incompatible with the v0.1.2 per-format feature gating.
//!
//! Detection is range-based on the Unicode codepoint per Unicode 15.1
//! Bidirectional Algorithm (UAX #9) strong RTL script blocks. The PDF
//! decoder may still consume `pdf_oxide`'s own implementation when
//! the `pdf` feature is enabled (richer character-class introspection
//! tied to the PDF rendering pipeline) ; the HTML decoder + any other
//! feature-agnostic site uses this module.

/// Returns `true` when the Unicode codepoint belongs to a script
/// whose default writing direction is right-to-left per Unicode
/// 15.1 §UAX #9 Bidirectional Algorithm.
///
/// Covers the major RTL script blocks (Hebrew / Arabic / Syriac /
/// Thaana / NKo / Samaritan / Mandaic / Aramaic / Kharoshthi) plus
/// the Arabic / Hebrew presentation forms used in legacy and
/// display-only contexts.
///
/// # Examples
///
/// ```
/// use olga::model::rtl::is_rtl_text;
///
/// // Hebrew aleph (א, U+05D0) — RTL.
/// assert!(is_rtl_text(0x05D0));
///
/// // Arabic alef (ا, U+0627) — RTL.
/// assert!(is_rtl_text(0x0627));
///
/// // Latin A (U+0041) — LTR.
/// assert!(!is_rtl_text(0x0041));
/// ```
#[must_use]
pub const fn is_rtl_text(ch: u32) -> bool {
    matches!(
        ch,
        // Hebrew (incl. Yiddish ligatures, points, marks)
        0x0590..=0x05FF
        // Arabic (incl. Persian, Urdu base block)
        | 0x0600..=0x06FF
        // Syriac
        | 0x0700..=0x074F
        // Arabic Supplement
        | 0x0750..=0x077F
        // Thaana (Maldivian)
        | 0x0780..=0x07BF
        // NKo (Mande languages of West Africa)
        | 0x07C0..=0x07FF
        // Samaritan
        | 0x0800..=0x083F
        // Mandaic
        | 0x0840..=0x085F
        // Arabic Extended-A (Quranic annotations, additional letters)
        | 0x08A0..=0x08FF
        // Hebrew Presentation Forms
        | 0xFB1D..=0xFB4F
        // Arabic Presentation Forms-A (ligatures, contextual variants)
        | 0xFB50..=0xFDFF
        // Arabic Presentation Forms-B (contextual final/initial/medial)
        | 0xFE70..=0xFEFF
        // Cypriot Syllabary
        | 0x10800..=0x1083F
        // Imperial Aramaic
        | 0x10840..=0x1085F
        // Kharoshthi
        | 0x10A00..=0x10A5F
    )
}

#[cfg(test)]
mod tests {
    use super::is_rtl_text;

    #[test]
    fn hebrew_aleph_is_rtl() {
        assert!(is_rtl_text(0x05D0));
    }

    #[test]
    fn arabic_alef_is_rtl() {
        assert!(is_rtl_text(0x0627));
    }

    #[test]
    fn latin_capital_a_is_ltr() {
        assert!(!is_rtl_text(0x0041));
    }

    #[test]
    fn cjk_unified_is_ltr() {
        // Chinese 中 (U+4E2D)
        assert!(!is_rtl_text(0x4E2D));
    }

    #[test]
    fn syriac_aleph_is_rtl() {
        assert!(is_rtl_text(0x0710));
    }

    #[test]
    fn thaana_letter_is_rtl() {
        assert!(is_rtl_text(0x0780));
    }

    #[test]
    fn nko_letter_is_rtl() {
        assert!(is_rtl_text(0x07C0));
    }

    #[test]
    fn arabic_presentation_form_b_is_rtl() {
        // FE8D — Arabic letter alef isolated form
        assert!(is_rtl_text(0xFE8D));
    }

    #[test]
    fn digit_zero_is_ltr() {
        assert!(!is_rtl_text(0x0030));
    }

    #[test]
    fn null_codepoint_is_ltr() {
        assert!(!is_rtl_text(0x0000));
    }
}
