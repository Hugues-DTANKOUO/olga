//! XLSX number-format engine.
//!
//! # Rendering entry point
//!
//! [`format_value`] is the sole rendering entry point. It consumes a raw
//! cell value plus the raw format-code string from the style profile
//! and dispatches through the POI-aligned pipeline:
//!
//! 1. [`section::split_format`] tokenises the format into up to four
//!    `CellFormatPart`-shaped sections, honouring quoted literals,
//!    bracket prefixes (`[Red]`, `[>=100]`, `[$-409]`), and elapsed-time
//!    markers (`[h]`, `[m]`, `[s]`).
//! 2. [`section::select_section_index`] picks the section for the value
//!    (condition-driven when any section carries a `[>=100]`-style
//!    condition, positional fall-through otherwise).
//! 3. The chosen section's [`SectionKind`](section::SectionKind) dispatches
//!    to one of [`number_formatter::format_number_body`],
//!    [`date_formatter::format_date_body`],
//!    [`date_formatter::format_elapsed_body`], or
//!    [`general_formatter::format_general`].
//!
//! # Legacy classifier (category-only consumers)
//!
//! The [`legacy`] module is retained as a category classifier only —
//! `CellNumberFormat` routes Default/Text short-circuits in
//! `sheets::raw` and keys the Date detection in
//! `cell_requires_workbook_decoder`. All numeric rendering goes through
//! [`format_value`].
//!
//! # Why a degradation contract
//!
//! See [`contract`] for the full rationale. Summary: the engine must
//! never silently emit a wrong rendering; whenever it falls back, it
//! attaches a [`FormatWarning`] so telemetry can measure coverage against
//! the POI and SSF corpora (~4000 assertions between them).

// The degradation contract and the POI-aligned entry point have been
// wired into the emission pass as of #47 (the watchlist validation
// task). Keeping the `contract` module `pub(super)` — the emit pass and
// the sheet renderer both import its types.
pub(super) mod contract;
mod legacy;
// POI-aligned section splitter + CellFormatPart parser. Consumed by
// `format_value` below — split_format + select_section_index drive the
// per-kind dispatch.
mod section;
// POI-aligned CellNumberFormatter reimplementation (#40). Consumes a
// Number-kind section body produced by `section::split_format` and
// renders against an abs-value plus a prepend_minus flag. Wired into
// `format_value` in the emit-pass swap in #47.
mod number_formatter;
// POI-aligned CellDateFormatter + CellElapsedFormatter reimplementation
// (#41). Consumes a Date- or Elapsed-kind section body produced by
// `section::split_format` and renders against an f64 serial plus a
// 1900-vs-1904 epoch flag. Wired into `format_value` in #47.
mod date_formatter;
// POI-aligned CellGeneralFormatter reimplementation (#42). Renders an
// f64 against Excel's "General" pseudo-format (adaptive 11-sig-digit
// rendering with scientific-notation fallback at magnitude extremes).
// Wired into `format_value` in #47; replaces the placeholder
// `format_f64_general` helper.
mod general_formatter;
// POI-aligned CellTextFormatter reimplementation (#42). Renders a
// Text-kind section body (the fourth section of a 4-section format, or
// a bare `@`) against the raw cell text. Wired into `format_value` in
// #47 for Text-kind sections; string-valued cells are still emitted
// directly by the sheet renderer because the raw XLSX path carries
// their text through `t="s"` / `t="inlineStr"` without passing through
// a format code.
#[allow(dead_code)]
mod text_formatter;
// POI `TestDataFormatter` fixture transliterations (#43). Pure
// assertion tables driving the per-kind formatters through a POI-shaped
// test-only dispatcher (split → select → kind dispatch). The companion
// task #44 adds the SheetJS SSF `test.tsv` corpus for cross-engine
// validation. See ATTRIBUTION.md for the Apache-2.0 derivation note on
// the fixture tables.
#[cfg(test)]
mod poi_fixtures;
// SheetJS SSF `test/` fixture transliterations (#44). Cross-engine validation
// against the second canonical Excel-rendering oracle. The module
// curates its assertions to the POI / SSF agreement zone and
// documents the two known divergence points (fixed-point digit
// budget, scientific-vs-fixed boundary at small magnitudes) so a
// future task can pick up the SSF-specific cases behind a feature
// flag.
#[cfg(test)]
mod ssf_fixtures;

// `parse_excel_number_format` and the `CellNumberFormat` enum survive
// from the original classifier implementation as the workbook-style category:
// `sheets::raw::read_raw_cell_content` short-circuits Default/Text back
// to the raw `value_text`, and `cell_requires_workbook_decoder` keys
// off Date for cell-type routing. Both consumers only need the coarse
// category — the raw format code goes through `format_value` below.
pub(crate) use legacy::{CellNumberFormat, parse_excel_number_format};

// Re-export the new contract types. These are plumbed into the emission
// pass via `format_value` below and by the bridge into the crate-wide
// `Warning` channel (see `push_format_warnings_into` /
// `format_warning_histogram` at the end of this module).
pub(crate) use contract::{FormatOptions, FormatWarning, FormatWarningKind, FormattedValue};

use std::collections::HashMap;

use crate::error::{Warning, WarningKind};
use date_formatter::{format_date_body, format_elapsed_body};
use general_formatter::format_general;
use number_formatter::format_number_body;
use section::{ParsedFormat, SectionKind, select_section_index, split_format};

/// POI-aligned formatter entry point.
///
/// Renders a raw cell value against a format code, returning a
/// [`FormattedValue`] that carries the displayed text plus any
/// [`FormatWarning`]s raised along the way.
///
/// # Pipeline
///
/// 1. [`split_format`] splits the code into up to four POI-style
///    [`FormatSection`](section::FormatSection)s, honouring
///    quoted literals and bracket prefixes.
/// 2. [`select_section_index`] picks the section that applies to
///    `value` (condition-driven when any section has a `[>=100]`-style
///    condition, positional fall-through otherwise).
/// 3. The chosen section's [`has_sign`](section::FormatSection) flag
///    decides whether the body carries its own sign literal: when
///    `has_sign` is true the caller feeds `|value|` to the body
///    formatter *without* prepending a minus (the body's literal `-`
///    or `(…)` renders the sign); when `has_sign` is false and the
///    value is negative, the caller prepends a minus itself. This is
///    the exact rule that fixes the `-+1bps` / `(-$5.00)` double-sign
///    defects.
/// 4. The body is dispatched by [`SectionKind`]: Number bodies go to
///    [`format_number_body`], Date bodies to [`format_date_body`],
///    Elapsed bodies to [`format_elapsed_body`], General to
///    [`format_general`], Text to the raw-text passthrough (for
///    numeric values the Text kind renders the float via General).
///
/// # Degradation contract
///
/// The engine never refuses to produce text. When the section splitter
/// fails to tokenise a malformed code, or when a per-kind formatter
/// returns an `Err` on grammar it cannot resolve (e.g. an unterminated
/// quoted literal, a trailing backslash, or a `/` fraction marker with
/// no numerator placeholder), the returned [`FormattedValue`] carries
/// the General-formatted text plus a [`FormatWarning`] describing the
/// degradation. Callers consume the warnings via
/// [`push_format_warnings_into`] to surface them into the crate-wide
/// `Warning` channel.
pub(crate) fn format_value(value: f64, format_code: &str, opts: &FormatOptions) -> FormattedValue {
    let trimmed = format_code.trim();
    // Empty / General sentinel short-circuits straight to the General
    // formatter. Section splitting an empty code would produce a single
    // empty Number section that renders as "" — worse than the explicit
    // "1234.5" General renders.
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("general") {
        return FormattedValue::clean(format_general(value));
    }

    let parsed: ParsedFormat = match split_format(format_code) {
        Ok(parsed) => parsed,
        Err(fw) => {
            // Malformed grammar — fall back to General, preserving the
            // original warning kind so telemetry can distinguish
            // MalformedFormatCode from UnhandledNumberFormat.
            return FormattedValue::with_warning(format_general(value), fw);
        }
    };

    if parsed.sections.is_empty() {
        return FormattedValue::clean(format_general(value));
    }

    let idx = select_section_index(&parsed, value);
    let section = &parsed.sections[idx];

    // POI's sign handling: when the body carries a literal `-` / `(` /
    // `)`, the formatter receives |value| and does NOT prepend a minus
    // (the body's literal draws the sign). Otherwise — i.e. no sign
    // literal in the body — a negative value prepends a minus.
    let abs_value = value.abs();
    let prepend_minus = value < 0.0 && !section.has_sign;

    match section.kind {
        SectionKind::Number => match format_number_body(abs_value, prepend_minus, &section.body) {
            Ok(text) => FormattedValue::clean(text),
            Err(fw) => FormattedValue::with_warning(format_general(value), fw),
        },
        SectionKind::Date => match format_date_body(abs_value, &section.body, opts.date_1904) {
            Ok(text) => FormattedValue::clean(text),
            Err(fw) => FormattedValue::with_warning(format_general(value), fw),
        },
        SectionKind::Elapsed => match format_elapsed_body(abs_value, &section.body) {
            Ok(text) => FormattedValue::clean(text),
            Err(fw) => FormattedValue::with_warning(format_general(value), fw),
        },
        SectionKind::General => FormattedValue::clean(format_general(value)),
        // Text-kind section on a numeric value: POI renders the numeric
        // via General (the `@` placeholder substitutes the raw cell text,
        // but a numeric cell has no raw text — the General renderer is
        // the nearest faithful form).
        SectionKind::Text => FormattedValue::clean(format_general(value)),
    }
}

// ---------------------------------------------------------------------------
// Warning-kind telemetry (#45)
// ---------------------------------------------------------------------------
//
// `FormatWarning` is the module-local telemetry channel: fine-grained,
// typed by `FormatWarningKind`, and enumerated specifically for the
// POI-aligned formatter's degradation modes. The crate-wide channel is
// `crate::error::Warning`, which is what `api::processability` already
// aggregates for downstream callers.
//
// These two helpers bridge the gap:
//
// * `push_format_warnings_into` converts each `FormatWarning` attached to
//   a `FormattedValue` into a `Warning` and pushes it onto the sheet-
//   pipeline's `Vec<Warning>`. The mapping mirrors the DOCX
//   `ScopedIssueKind` → `WarningKind` bridge in
//   `formats::docx::body::warnings`: the module-local kind is lost in
//   translation (the public enum stays stable and C-like) but is preserved
//   as a `[tag]` prefix in the message so tests, CI diff runs, and future
//   telemetry dashboards can recover the sub-kind by substring match.
//
// * `format_warning_histogram` groups a slice of `FormatWarning`s by
//   `FormatWarningKind`. This is the primary CI assertion surface: once
//   `format_value` is wired into the emit pass in #46, we will run the
//   POI and SSF corpora through the full extractor and assert that the
//   histogram for specific format-kind buckets trends toward zero as each
//   feature ships.
//
// Both helpers are `pub(crate)` and deliberately dead-coded today: the
// emit pass that will call them lands in #46 in lockstep with the
// comments-on-empty-cells fix. Wiring them now — rather than lumping
// them into #46 — keeps the per-feature formatter tasks testable against
// the public `Warning` channel in isolation, and stops #46 from
// conflating "flip the pipeline to `format_value`" with "teach the
// crate-wide warning channel about format codes".

/// Bridge every [`FormatWarning`] on a [`FormattedValue`] into the
/// crate-wide `Vec<Warning>` the XLSX sheet pipeline threads through
/// [`process_sheets`](super::sheets::process_sheets).
///
/// Produces one [`Warning`] per [`FormatWarning`] in insertion order.
/// The fine-grained [`FormatWarningKind`] is encoded into the message
/// prefix (`[unhandled_number_format]`, `[malformed_format_code]`, …)
/// so substring matching in tests and telemetry dashboards can recover
/// the sub-kind without needing a new public enum variant per feature.
///
/// `page` mirrors the sheet index already attached to every sibling
/// XLSX warning (see e.g. `build_sheet_processing_plan` in
/// `formats::xlsx::sheets`).
pub(crate) fn push_format_warnings_into(
    fv: &FormattedValue,
    page: Option<u32>,
    sink: &mut Vec<Warning>,
) {
    for fw in &fv.warnings {
        sink.push(format_warning_to_warning(fw, page));
    }
}

/// Project a slice of [`FormatWarning`]s onto a histogram keyed by
/// [`FormatWarningKind`].
///
/// Intended for CI assertions against the POI / SSF corpora ("running
/// the POI fixture suite must not emit any `MalformedFormatCode`
/// warnings") and for per-document diagnostics ("this workbook hit
/// `FormatFeatureFraction` 17 times"). Buckets with a zero count are
/// omitted — iterate over [`FormatWarningKind`] explicitly if absent
/// buckets need to be distinguished from truly empty ones.
#[allow(dead_code)] // wired into the emit pass in #46
pub(crate) fn format_warning_histogram(
    warnings: &[FormatWarning],
) -> HashMap<FormatWarningKind, u32> {
    let mut hist: HashMap<FormatWarningKind, u32> = HashMap::new();
    for w in warnings {
        *hist.entry(w.kind).or_insert(0) += 1;
    }
    hist
}

/// Project a single [`FormatWarning`] onto the crate-wide
/// [`Warning`] shape.
///
/// Kept private because the only caller is
/// [`push_format_warnings_into`]. Split out so the per-kind mapping is
/// unit-testable without constructing a full [`FormattedValue`].
fn format_warning_to_warning(fw: &FormatWarning, page: Option<u32>) -> Warning {
    Warning {
        kind: warning_kind_for(fw.kind),
        message: format!(
            "xlsx number format [{}]: {} (format_code={:?}, value={:?})",
            format_warning_kind_tag(fw.kind),
            fw.message,
            fw.format_code,
            fw.raw_value,
        ),
        page,
    }
}

/// Map a [`FormatWarningKind`] onto the nearest public
/// [`WarningKind`] bucket.
///
/// The public enum is intentionally C-like and stable (see
/// `crate::error::WarningKind`): every grammar feature we can't render
/// maps to [`WarningKind::PartialExtraction`] — it's a faithful
/// rendering we could not produce — except for the tokenisation failure
/// case, which maps to [`WarningKind::MalformedContent`] because the
/// format code is structurally broken. The module-local sub-kind
/// survives as a `[tag]` prefix in the message emitted by
/// [`format_warning_to_warning`], so downstream filtering can still
/// separate the buckets without breaking existing telemetry
/// aggregations (e.g. [`crate::api::processability`]).
fn warning_kind_for(kind: FormatWarningKind) -> WarningKind {
    match kind {
        FormatWarningKind::UnhandledNumberFormat
        | FormatWarningKind::FormatFeatureFraction
        | FormatWarningKind::FormatFeatureConditional
        | FormatWarningKind::FormatFeatureFill
        | FormatWarningKind::FormatFeatureLocale
        | FormatWarningKind::FormatFeatureColor => WarningKind::PartialExtraction,
        FormatWarningKind::MalformedFormatCode => WarningKind::MalformedContent,
    }
}

/// Stable snake_case tag for a [`FormatWarningKind`] that callers can
/// grep for in emitted [`Warning`] messages. Renaming a tag is a
/// backwards-incompatible change to the `Warning.message` contract, so
/// this map is intentionally append-only.
fn format_warning_kind_tag(kind: FormatWarningKind) -> &'static str {
    match kind {
        FormatWarningKind::UnhandledNumberFormat => "unhandled_number_format",
        FormatWarningKind::MalformedFormatCode => "malformed_format_code",
        FormatWarningKind::FormatFeatureFraction => "feature_fraction",
        FormatWarningKind::FormatFeatureConditional => "feature_conditional",
        FormatWarningKind::FormatFeatureFill => "feature_fill",
        FormatWarningKind::FormatFeatureLocale => "feature_locale",
        FormatWarningKind::FormatFeatureColor => "feature_color",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The scaffold must be byte-identical to the legacy path for every
    /// format code the legacy path can render. This pins the "no
    /// regression during the rewrite" invariant.
    #[test]
    fn format_value_matches_legacy_for_percent() {
        let opts = FormatOptions::default();
        let fv = format_value(-0.0261111, "0.0%", &opts);
        assert_eq!(fv.text, "-2.6%");
        assert!(fv.warnings.is_empty());
    }

    #[test]
    fn format_value_matches_legacy_for_currency() {
        let opts = FormatOptions::default();
        let fv = format_value(450_000.0, "$#,##0", &opts);
        assert_eq!(fv.text, "$450,000");
        assert!(fv.warnings.is_empty());
    }

    #[test]
    fn format_value_matches_legacy_for_date() {
        let opts = FormatOptions::default();
        let fv = format_value(46115.0, "yyyy-mm-dd", &opts);
        assert_eq!(fv.text, "2026-04-03");
        assert!(fv.warnings.is_empty());
    }

    /// The exact watchlist canaries: format codes the legacy classifier
    /// collapsed to Default now render faithfully via the POI dispatch.
    /// Pinning each one here means a regression in the per-kind
    /// formatters surfaces at the dispatch boundary (not deep in a
    /// corpus test).
    #[test]
    fn format_value_renders_scientific_notation_faithfully() {
        // `0.00E+00` on 2.918e13 used to render `29180000000000.00`.
        // POI-aligned: `2.92E+13`.
        let opts = FormatOptions::default();
        let fv = format_value(2.918e13, "0.00E+00", &opts);
        assert_eq!(fv.text, "2.92E+13");
        assert!(
            fv.warnings.is_empty(),
            "unexpected warnings: {:?}",
            fv.warnings
        );
    }

    #[test]
    fn format_value_renders_literal_suffix_faithfully() {
        // `0.0"x"` — a single-decimal number with the literal `x`
        // suffix. Previously collapsed to `Currency` with ambiguous
        // suffix trimming.
        let opts = FormatOptions::default();
        let fv = format_value(62.4, "0.0\"x\"", &opts);
        assert_eq!(fv.text, "62.4x");
        assert!(fv.warnings.is_empty());
    }

    #[test]
    fn format_value_renders_custom_sign_bearing_negative_without_double_sign() {
        // `+0"bps";-0"bps"` — the watchlist format that used to render
        // `-+1bps` for -1.0. POI's hasSign() sees the `-` in the negative
        // section body, passes |value| + prepend_minus=false, and the
        // body's literal `-` draws the sign exactly once.
        let opts = FormatOptions::default();
        let fv = format_value(-1.0, "+0\"bps\";-0\"bps\"", &opts);
        assert_eq!(fv.text, "-1bps");
        assert!(fv.warnings.is_empty());
    }

    #[test]
    fn format_value_renders_custom_sign_bearing_positive() {
        // Same format applied to +2.0 — the positive section's `+`
        // literal is not a sign (has_sign tracks `-`, `(`, `)` only),
        // so the body renders verbatim.
        let opts = FormatOptions::default();
        let fv = format_value(2.0, "+0\"bps\";-0\"bps\"", &opts);
        assert_eq!(fv.text, "+2bps");
        assert!(fv.warnings.is_empty());
    }

    #[test]
    fn format_value_renders_h_mm_am_pm_as_time_of_day() {
        // `h:mm AM/PM` on the fraction 9:30 (9.5/24) used to render
        // `1899-12-31T09:30:00` — the legacy classifier's Date bucket
        // went through ISO formatting. POI's Date kind parses the
        // `AM/PM` token and renders the time-of-day.
        let opts = FormatOptions::default();
        let fv = format_value(9.0 / 24.0 + 30.0 / 1440.0, "h:mm AM/PM", &opts);
        assert_eq!(fv.text, "9:30 AM");
        assert!(fv.warnings.is_empty());
    }

    #[test]
    fn general_format_renders_via_poi_general_formatter() {
        let opts = FormatOptions::default();
        let fv = format_value(1234.5, "General", &opts);
        assert_eq!(fv.text, "1234.5");
        assert!(fv.warnings.is_empty());
    }

    #[test]
    fn empty_format_renders_via_poi_general_formatter() {
        let opts = FormatOptions::default();
        let fv = format_value(1234.5, "", &opts);
        assert_eq!(fv.text, "1234.5");
        assert!(fv.warnings.is_empty());
    }

    #[test]
    fn malformed_format_code_falls_back_to_general_with_warning() {
        // Unterminated bracket — the section splitter still refuses
        // this because `[...]` is structural (conditions, colors,
        // locales) and there's no sensible fallback. Unterminated
        // quotes, by contrast, are tokenised leniently: see the
        // `# ?/8"` manifest path which must render, not fail.
        let opts = FormatOptions::default();
        let fv = format_value(42.0, "[>=100 0.0", &opts);
        assert_eq!(fv.text, "42");
        assert_eq!(fv.warnings.len(), 1);
        assert_eq!(fv.warnings[0].kind, FormatWarningKind::MalformedFormatCode);
    }

    // -----------------------------------------------------------------------
    // Warning-kind telemetry bridge (#45)
    // -----------------------------------------------------------------------

    /// `warning_kind_for` is the public-API contract: rename it freely,
    /// but every variant of `FormatWarningKind` must map to a valid
    /// `WarningKind`. Pin the per-variant mapping here so a future
    /// reshuffle of the variant set surfaces as a test failure rather
    /// than a silent telemetry drift.
    #[test]
    fn every_format_warning_kind_maps_to_a_warning_kind() {
        let cases: &[(FormatWarningKind, WarningKind)] = &[
            (
                FormatWarningKind::UnhandledNumberFormat,
                WarningKind::PartialExtraction,
            ),
            (
                FormatWarningKind::MalformedFormatCode,
                WarningKind::MalformedContent,
            ),
            (
                FormatWarningKind::FormatFeatureFraction,
                WarningKind::PartialExtraction,
            ),
            (
                FormatWarningKind::FormatFeatureConditional,
                WarningKind::PartialExtraction,
            ),
            (
                FormatWarningKind::FormatFeatureFill,
                WarningKind::PartialExtraction,
            ),
            (
                FormatWarningKind::FormatFeatureLocale,
                WarningKind::PartialExtraction,
            ),
            (
                FormatWarningKind::FormatFeatureColor,
                WarningKind::PartialExtraction,
            ),
        ];
        for &(format_kind, expected) in cases {
            assert_eq!(
                warning_kind_for(format_kind),
                expected,
                "{:?} should bridge to {:?}",
                format_kind,
                expected,
            );
        }
    }

    /// The snake_case tag is the substring contract callers grep for in
    /// the bridged `Warning.message`. Pin every variant — renaming a tag
    /// is a backwards-incompatible change.
    #[test]
    fn every_format_warning_kind_has_a_stable_tag() {
        let cases: &[(FormatWarningKind, &str)] = &[
            (
                FormatWarningKind::UnhandledNumberFormat,
                "unhandled_number_format",
            ),
            (
                FormatWarningKind::MalformedFormatCode,
                "malformed_format_code",
            ),
            (FormatWarningKind::FormatFeatureFraction, "feature_fraction"),
            (
                FormatWarningKind::FormatFeatureConditional,
                "feature_conditional",
            ),
            (FormatWarningKind::FormatFeatureFill, "feature_fill"),
            (FormatWarningKind::FormatFeatureLocale, "feature_locale"),
            (FormatWarningKind::FormatFeatureColor, "feature_color"),
        ];
        for &(format_kind, expected) in cases {
            assert_eq!(format_warning_kind_tag(format_kind), expected);
        }
    }

    /// `push_format_warnings_into` produces one `Warning` per attached
    /// `FormatWarning`, in insertion order, with the page propagated. A
    /// `FormattedValue` with no warnings must leave the sink untouched —
    /// the happy path through the emit pass should not allocate.
    #[test]
    fn push_format_warnings_into_bridges_each_attached_warning() {
        let mut sink: Vec<Warning> = Vec::new();
        let fv = FormattedValue::clean("1,234.5");
        push_format_warnings_into(&fv, Some(0), &mut sink);
        assert!(sink.is_empty(), "clean values must not push warnings");

        let mut fv = FormattedValue::clean("fallback");
        fv.warnings.push(FormatWarning::new(
            FormatWarningKind::UnhandledNumberFormat,
            "0.00E+00",
            "29180000000000",
            "scientific notation not yet supported",
        ));
        fv.warnings.push(FormatWarning::new(
            FormatWarningKind::FormatFeatureFraction,
            "# ?/?",
            "0.5",
            "fraction grammar not yet supported",
        ));

        push_format_warnings_into(&fv, Some(2), &mut sink);
        assert_eq!(sink.len(), 2);
        assert_eq!(sink[0].kind, WarningKind::PartialExtraction);
        assert_eq!(sink[0].page, Some(2));
        assert!(
            sink[0].message.contains("[unhandled_number_format]"),
            "tag prefix lost: {}",
            sink[0].message
        );
        assert!(
            sink[0].message.contains("0.00E+00"),
            "format_code missing from message: {}",
            sink[0].message
        );
        assert!(
            sink[0].message.contains("29180000000000"),
            "raw_value missing from message: {}",
            sink[0].message
        );
        assert_eq!(sink[1].kind, WarningKind::PartialExtraction);
        assert!(sink[1].message.contains("[feature_fraction]"));
    }

    /// Malformed format codes bridge to `MalformedContent`, not
    /// `PartialExtraction`. This is the one mapping that diverges from
    /// the default bucket — pinned separately because conflating it with
    /// the default would mask "we couldn't even tokenise the format
    /// code" inside the much-noisier "we don't render this feature yet"
    /// bucket.
    #[test]
    fn malformed_format_code_bridges_to_malformed_content() {
        let mut sink: Vec<Warning> = Vec::new();
        let mut fv = FormattedValue::clean("");
        fv.warnings.push(FormatWarning::new(
            FormatWarningKind::MalformedFormatCode,
            "0.00E+00\"unterminated",
            "1.0",
            "unterminated quoted literal",
        ));

        push_format_warnings_into(&fv, None, &mut sink);
        assert_eq!(sink.len(), 1);
        assert_eq!(sink[0].kind, WarningKind::MalformedContent);
        assert!(sink[0].message.contains("[malformed_format_code]"));
        assert_eq!(sink[0].page, None);
    }

    /// The histogram is the primary CI surface for tracking POI / SSF
    /// coverage as each per-feature reimplementation lands. Pin the basic shape
    /// (count by kind, omit empty buckets) so a refactor of the
    /// implementation can't silently change the dashboard semantics.
    #[test]
    fn format_warning_histogram_counts_by_kind() {
        let warnings = vec![
            FormatWarning::new(
                FormatWarningKind::UnhandledNumberFormat,
                "0.00E+00",
                "1.0",
                "scientific",
            ),
            FormatWarning::new(
                FormatWarningKind::UnhandledNumberFormat,
                "0.0\"x\"",
                "62.4",
                "literal suffix",
            ),
            FormatWarning::new(
                FormatWarningKind::FormatFeatureFraction,
                "# ?/?",
                "0.5",
                "fraction",
            ),
        ];

        let hist = format_warning_histogram(&warnings);
        assert_eq!(hist[&FormatWarningKind::UnhandledNumberFormat], 2);
        assert_eq!(hist[&FormatWarningKind::FormatFeatureFraction], 1);
        // Empty buckets are omitted.
        assert!(!hist.contains_key(&FormatWarningKind::MalformedFormatCode));
        assert!(!hist.contains_key(&FormatWarningKind::FormatFeatureColor));
    }

    /// An empty input slice yields an empty histogram (no panic, no
    /// pre-seeded zero buckets). Pin the property because the emit
    /// pass will call this on every cell, including the happy path.
    #[test]
    fn format_warning_histogram_handles_empty_input() {
        let hist = format_warning_histogram(&[]);
        assert!(hist.is_empty());
    }
}
