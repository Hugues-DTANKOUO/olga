//! Degradation contract for the XLSX number-format engine.
//!
//! ## Why this exists
//!
//! Excel's number-format grammar (ECMA-376 §18.8.30) is large — literal runs,
//! four-section dispatch, conditional sections (`[>=100]`), colour codes,
//! locale codes (`[$-409]`), elapsed-time markers (`[h]`, `[m]`, `[s]`),
//! padding directives (`_x`), fill directives (`*x`), fractions (`# ?/?`),
//! and General auto-formatting with up to 11 significant digits. The gold-
//! standard implementation (Apache POI's `CellFormat` + `CellFormatPart` +
//! the five `CellXxxFormatter` siblings) runs to ~3 kLOC of Java.
//!
//! For olga's mass-market target, one property matters more than feature
//! coverage: **we must never silently emit a wrong rendering**. If the
//! engine cannot faithfully render a value, it says so — structurally —
//! and the emission pass decides whether to fall back to the raw value,
//! surface a partial rendering, or abort. The previous calamine-based
//! pipeline violated this invariant on memoized real-world documents
//! (watchlist.xlsx): `0.00E+00` collapsed to `29180000000000.00`, the
//! custom bps format `+0"bps";-0"bps"` emitted `-+1bps` because the engine
//! both stripped the section dispatch and added its own minus sign.
//!
//! ## The contract
//!
//! A single entry point — [`format_value`] — returns a [`FormattedValue`].
//! The success type always carries a `text` field (what the extractor will
//! put into the cell primitive) and an accumulated list of [`FormatWarning`]
//! that describe any degradation that occurred during formatting. A
//! [`FormatWarning`] carries the specific [`FormatWarningKind`], the
//! offending `format_code`, and the `raw_value` so downstream code can
//! count by kind, diff against fixtures, and reproduce the failure.
//!
//! The `Result<_, FormatWarning>` return shape is reserved for the case
//! where the engine cannot produce *any* output at all (e.g. a malformed
//! format code we can't even tokenise). Whenever a reasonable text
//! rendering is available — even if it's a fallback to the raw numeric —
//! the engine returns `Ok(FormattedValue { text, warnings })` so the
//! emission pass keeps the cell and the telemetry pass still counts the
//! degradation.
//!
//! ## Industry alignment
//!
//! POI's `CellFormat.apply()` returns `CellFormatResult { text, textColor }`
//! and never throws on grammar it doesn't support — it falls back to the
//! raw value. SheetJS SSF's `format()` returns a plain `String` but records
//! unhandled tokens by falling back to a `General`-style render. Our
//! contract is the intersection of both, plus a structured warning channel
//! so we can measure coverage against the POI and SSF test corpora as
//! features land.

/// Calendar options for a single [`format_value`] call.
///
/// Instantiated once per sheet so the workbook's `date1904` flag travels
/// with every cell render. Extending this struct is the correct place to
/// add future knobs (currency negotiation rules, locale-specific grouping
/// separators) without perturbing call sites.
#[derive(Debug, Clone, Default)]
pub(crate) struct FormatOptions {
    /// `true` iff the workbook uses the 1904 date system (macOS Excel
    /// pre-2011). OOXML signals this via `workbookPr/@date1904`.
    pub date_1904: bool,
}

/// Successful rendering — text payload plus any degradation warnings.
///
/// A caller must always consume `warnings` (empty is the happy path)
/// before relying on `text`: telemetry branches on warning kinds to
/// decide whether the extraction remains within contract. The shape —
/// a rendered text payload plus a structured warning channel — plays
/// the same role as POI's `CellFormatResult`, but the warning list
/// replaces POI's log-line reporting with a typed, queryable stream.
#[derive(Debug, Clone)]
pub(crate) struct FormattedValue {
    /// User-facing displayed text for the cell.
    pub text: String,

    /// Degradation events accumulated while formatting this value.
    ///
    /// Empty on the happy path. A non-empty list means the engine
    /// encountered a grammar feature it could not render faithfully
    /// and fell back to a safe default (General-style render or the
    /// raw numeric), so `text` still contains something meaningful —
    /// it just isn't what Excel would have drawn.
    pub warnings: Vec<FormatWarning>,
}

impl FormattedValue {
    /// Convenience: a clean rendering with no degradation.
    pub(crate) fn clean(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            warnings: Vec::new(),
        }
    }

    /// Convenience: a rendering that carries a single degradation warning.
    pub(crate) fn with_warning(text: impl Into<String>, warning: FormatWarning) -> Self {
        Self {
            text: text.into(),
            warnings: vec![warning],
        }
    }
}

/// Structured degradation event — raised whenever the engine cannot
/// faithfully render a cell's value against its format code.
///
/// Carries the full reproduction context (the offending `format_code`,
/// the raw stringified value, the specific [`FormatWarningKind`]) so
/// downstream telemetry, CI diff tests, and failure triage can count and
/// re-run individual cells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FormatWarning {
    pub kind: FormatWarningKind,
    pub format_code: String,
    pub raw_value: String,
    pub message: String,
}

impl FormatWarning {
    pub(crate) fn new(
        kind: FormatWarningKind,
        format_code: impl Into<String>,
        raw_value: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            format_code: format_code.into(),
            raw_value: raw_value.into(),
            message: message.into(),
        }
    }
}

/// Classification of a single formatting failure — the telemetry key.
///
/// Each variant maps 1:1 to a `WarningKind::Xxx` emitted into the
/// decoder's `warnings` vec, so CI can assert on coverage (e.g. "no
/// `MalformedFormatCode` on the POI corpus"). Variants are intentionally
/// stable and additive: new grammar features get a new kind, so existing
/// telemetry dashboards don't break when we tighten coverage.
///
/// A subset of variants (`UnhandledNumberFormat`,
/// `FormatFeatureConditional`, `FormatFeatureFill`, `FormatFeatureLocale`,
/// `FormatFeatureColor`) is reserved: today's POI dispatch never emits
/// them because the section splitter absorbs the corresponding grammar
/// features without refusing to format. `FormatFeatureFraction` is also
/// rare in practice — well-formed fraction codes render natively, so the
/// variant only appears for syntactically broken `/` markers. These kinds
/// are kept as histogram keys so CI suites and external dashboards
/// addressing them by name (via
/// [`format_warning_kind_tag`](super::format_warning_kind_tag)) remain
/// forward-compatible; the `#[allow(dead_code)]` lets `-D warnings` co-
/// exist with the stability contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub(crate) enum FormatWarningKind {
    /// The format code tokenises but contains a category we don't yet
    /// format (e.g. a new grammar feature reaching this sink before
    /// the dedicated formatter ships). Fallback: General-style render.
    UnhandledNumberFormat,

    /// The format code is syntactically broken — unterminated quoted
    /// literal, unmatched bracket, etc. Fallback: General-style render.
    MalformedFormatCode,

    /// The format code contains a `/` fraction marker the parser could
    /// not resolve into a `numerator / denominator` pair — e.g. `/8` with
    /// no numerator placeholder, or `?/0`. Well-formed fraction codes
    /// (`# ?/?`, `?/8`, `# ??/??`, …) render natively via
    /// [`CellNumberFormatter`] and do not raise this warning; the variant
    /// is kept so CI can still count genuine malformations and so the
    /// `feature_fraction` histogram key stays stable.
    FormatFeatureFraction,

    /// The format code contains a conditional section (`[>=100]0.0`).
    /// Fallback: apply the matching section in order (POI's behaviour)
    /// but warn so we can eventually validate selection logic.
    FormatFeatureConditional,

    /// The format code contains a fill directive (`*x`) that repeats a
    /// character to fill the column width. Fallback: drop the fill
    /// (we don't know the column width at extraction time anyway).
    FormatFeatureFill,

    /// The format code references a locale (`[$-409]`) or currency
    /// (`[$$-409]`) we don't yet recognise. Fallback: preserve the
    /// literal and warn.
    FormatFeatureLocale,

    /// The format code contains a colour code (`[Red]`, `[Blue]`).
    /// Fallback: drop the colour (we don't emit styling) and render
    /// the rest of the section.
    FormatFeatureColor,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_carries_no_warnings() {
        let fv = FormattedValue::clean("9:30 AM");
        assert_eq!(fv.text, "9:30 AM");
        assert!(fv.warnings.is_empty());
    }

    #[test]
    fn with_warning_preserves_text_and_attaches_warning() {
        let warning = FormatWarning::new(
            FormatWarningKind::UnhandledNumberFormat,
            "0.0\"x\"",
            "62.4",
            "literal suffix \"x\" not yet supported",
        );
        let fv = FormattedValue::with_warning("62.4", warning.clone());
        assert_eq!(fv.text, "62.4");
        assert_eq!(fv.warnings, vec![warning]);
    }

    #[test]
    fn warning_kinds_are_equatable_for_telemetry_histograms() {
        // The histogram in the emission pass relies on PartialEq/Hash
        // to count warnings by kind; pin the property.
        use std::collections::HashMap;
        let mut hist: HashMap<FormatWarningKind, u32> = HashMap::new();
        *hist
            .entry(FormatWarningKind::UnhandledNumberFormat)
            .or_insert(0) += 1;
        *hist
            .entry(FormatWarningKind::UnhandledNumberFormat)
            .or_insert(0) += 1;
        *hist
            .entry(FormatWarningKind::FormatFeatureFraction)
            .or_insert(0) += 1;

        assert_eq!(hist[&FormatWarningKind::UnhandledNumberFormat], 2);
        assert_eq!(hist[&FormatWarningKind::FormatFeatureFraction], 1);
    }
}
