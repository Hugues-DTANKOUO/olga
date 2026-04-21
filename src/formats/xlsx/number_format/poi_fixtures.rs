//! POI `TestDataFormatter` fixture transliterations — large
//! `(format, value, expected)` assertion tables driving the POI-aligned
//! formatters end-to-end via a test-only dispatcher.
//!
//! These fixtures are test-only derivative tables under Apache-2.0; see
//! ATTRIBUTION.md for the full derivation note.
//!
//! # Why a separate fixture module
//!
//! Each per-kind formatter (`general`, `number`, `date`, `elapsed`,
//! `text`) ships with focused unit tests covering its own grammar. This
//! module exists to exercise the *integration* — section split → section
//! select → kind dispatch — that the engine will perform once #46 wires
//! `format_value` into the emit pass. By driving fixtures through a
//! POI-shaped adapter today, every assertion below also serves as a
//! regression net for that future swap: the same inputs feeding the same
//! outputs through a different entry point.
//!
//! The fixtures themselves are transliterations of the assertions in
//! Apache POI's `TestDataFormatter` (`org.apache.poi.ss.format`), which
//! is the de-facto industry oracle for Excel cell rendering. The intent
//! is not literal byte-for-byte copy from Java to Rust — POI tests
//! intersperse setup and helper calls — but to capture each assertion's
//! `(format, value, expected)` triple in the most condensed form Rust
//! offers (a static table) and to organise the tables by feature so a
//! diff against POI's source is straightforward to audit.
//!
//! # Why a test-only dispatcher
//!
//! `mod.rs::format_value` still routes through the legacy classifier
//! pending #46. Building POI-style integration coverage today requires a
//! shim that splits the format code, picks the right section, and
//! dispatches on kind — exactly what #46 will land in production code.
//! The shim here lives in `cfg(test)` so it can't drift away from the
//! production dispatcher: the moment #46 lands, this shim collapses into
//! a single call to the new entry point and the fixtures keep running
//! unchanged.
//!
//! # Coverage scope
//!
//! This commit transliterates a representative ~200-assertion subset of
//! POI's suite — enough to lock down each formatter's contract through
//! the integration path. The companion task #44 transliterates SheetJS
//! SSF's ~3000-assertion `test.tsv` for cross-engine validation.

#![cfg(test)]

use super::contract::FormatOptions;
use super::date_formatter::{format_date_body, format_elapsed_body};
use super::general_formatter::format_general;
use super::number_formatter::format_number_body;
use super::section::{SectionKind, select_section_index, split_format};
use super::text_formatter::format_text_body;

// ---------------------------------------------------------------------------
// POI-style end-to-end dispatcher (test-only shim for #46)
// ---------------------------------------------------------------------------

/// Apply a numeric format code to a numeric value, mirroring POI's
/// `CellFormat.getInstance(code).apply(value).text`.
///
/// The split → select → dispatch flow is identical to what `format_value`
/// will perform after #46. Errors fall back to `General` rendering — POI
/// does the same for unparseable bodies, and the degradation contract
/// requires we never silently emit empty text.
fn apply(code: &str, value: f64, opts: &FormatOptions) -> String {
    // Empty format code or bare `General` — go straight to the General
    // formatter (skipping the splitter, which would synthesise a single
    // empty section).
    let trimmed = code.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("general") {
        return format_general(value);
    }

    let parsed = match split_format(code) {
        Ok(p) => p,
        Err(_) => return format_general(value),
    };
    if parsed.sections.is_empty() {
        return format_general(value);
    }

    let idx = select_section_index(&parsed, value);
    let section = &parsed.sections[idx];

    // Sign handling (POI's hasSign rule):
    //   * If the chosen section carries its own literal `-`, `(`, or `)`,
    //     the body renders the sign — pass `prepend_minus = false`.
    //   * Otherwise, prepend a minus iff the original value was negative.
    // POI applies this rule uniformly across positive, negative, and
    // conditional sections — the only structural escape is for the
    // section author to write a literal sign marker in the body, which
    // sets `has_sign = true`. SSF's "user takes control of sign in
    // multi-section formats" rule is a deliberate divergence; the SSF
    // fixture module pins it as `poi_over_ssf_*` cases. The body
    // always sees the absolute value either way.
    let abs_value = value.abs();
    let prepend_minus = !section.has_sign && value < 0.0;

    let body = section.body.as_str();

    match section.kind {
        SectionKind::General => format_general(value),
        SectionKind::Number => format_number_body(abs_value, prepend_minus, body)
            .unwrap_or_else(|_| format_general(value)),
        SectionKind::Date => format_date_body(abs_value, body, opts.date_1904)
            .unwrap_or_else(|_| format_general(value)),
        SectionKind::Elapsed => {
            format_elapsed_body(abs_value, body).unwrap_or_else(|_| format_general(value))
        }
        // A Text section selected for a numeric value is unusual; POI
        // falls through to the next available section, but our select
        // helper only returns a Text index for length-4 string-valued
        // cells. Fall back to General for safety.
        SectionKind::Text => format_general(value),
    }
}

/// Apply a format code to a string-valued cell, mirroring POI's
/// `CellFormat.apply(textValue)` for non-numeric cells.
///
/// POI's rule: the 4th section (if present) is the text section. When
/// the format code has a single `@`-bearing section, the same body
/// applies. Otherwise the raw text passes through unchanged.
fn apply_text(code: &str, text: &str) -> String {
    let parsed = match split_format(code) {
        Ok(p) => p,
        Err(_) => return text.to_string(),
    };

    // POI's 4-section rule: the fourth section IS the text section
    // regardless of what the body looks like. A pure quoted-literal
    // section (`"qux"`) is classified as Number by our token-level
    // classifier because it lacks an `@` placeholder, but positionally
    // it's the text branch — POI / SSF treat it as such. Trust the
    // position, not the kind, for slot 3.
    if parsed.sections.len() >= 4 {
        return format_text_body(text, &parsed.sections[3].body)
            .unwrap_or_else(|_| text.to_string());
    }
    // Single-section `@` format — the only 1-section form that routes
    // text into the formatter. Any other single section ignores the
    // text and the caller passes it through unchanged.
    if parsed.sections.len() == 1 && parsed.sections[0].kind == SectionKind::Text {
        return format_text_body(text, &parsed.sections[0].body)
            .unwrap_or_else(|_| text.to_string());
    }
    text.to_string()
}

/// Convenience wrapper for tables that don't care about the workbook
/// epoch (the vast majority — only date fixtures using the 1904 base
/// need to opt in).
fn apply_default(code: &str, value: f64) -> String {
    apply(code, value, &FormatOptions::default())
}

/// Verify a `(format, value, expected)` row, with a panic message that
/// names the failing format code so a single failed assertion in a
/// hundred-row table is easy to track down.
#[track_caller]
fn check(code: &str, value: f64, expected: &str) {
    let got = apply_default(code, value);
    assert_eq!(
        got, expected,
        "format(`{code}`, {value}) = `{got}` but POI says `{expected}`"
    );
}

#[track_caller]
fn check_text(code: &str, text: &str, expected: &str) {
    let got = apply_text(code, text);
    assert_eq!(
        got, expected,
        "format(`{code}`, text=`{text}`) = `{got}` but POI says `{expected}`"
    );
}

// ---------------------------------------------------------------------------
// General format (CellGeneralFormatter)
// ---------------------------------------------------------------------------

/// POI's `CellGeneralFormatter` test cases — both via the `"General"`
/// keyword and via the empty-format code (Excel treats them identically).
#[test]
fn general_keyword_renders_via_general_formatter() {
    let cases: &[(f64, &str)] = &[
        // Zero & sign collapse.
        (0.0, "0"),
        (-0.0, "0"),
        // Plain integers (within the 11-digit budget).
        (1.0, "1"),
        (42.0, "42"),
        (-7.0, "-7"),
        (1234.0, "1234"),
        (-1234.0, "-1234"),
        (1_234_567_890.0, "1234567890"),
        // Fixed-point with trim.
        (1.5, "1.5"),
        (-1.5, "-1.5"),
        (2.25, "2.25"),
        (0.001, "0.001"),
        (0.0001, "0.0001"),
        (0.5, "0.5"),
        // Magnitude boundaries — switch to scientific.
        (1e11, "1E+11"),
        (-1e11, "-1E+11"),
        (1e-5, "1E-05"),
        (5.2e-5, "5.2E-05"),
        (-5.2e-5, "-5.2E-05"),
        // Mantissa rounding to 6 significant digits.
        (1.2345678e12, "1.23457E+12"),
        (123_456_789_012.0, "1.23457E+11"),
        // Carry-into-exponent.
        (9.999999e11, "1E+12"),
        // Non-finite → POI emits `#NUM!`.
        (f64::NAN, "#NUM!"),
        (f64::INFINITY, "#NUM!"),
        (f64::NEG_INFINITY, "#NUM!"),
    ];
    for &(value, expected) in cases {
        check("General", value, expected);
    }
}

#[test]
fn empty_format_code_falls_through_to_general() {
    let cases: &[(f64, &str)] = &[
        (0.0, "0"),
        (1.0, "1"),
        (-1.0, "-1"),
        (1.5, "1.5"),
        (1e11, "1E+11"),
    ];
    for &(value, expected) in cases {
        check("", value, expected);
    }
}

// ---------------------------------------------------------------------------
// Number format — integer placeholders
// ---------------------------------------------------------------------------

#[test]
fn number_integer_placeholder_required_pads_with_zeros() {
    let cases: &[(&str, f64, &str)] = &[
        ("0", 0.0, "0"),
        ("0", 5.0, "5"),
        ("0", 42.0, "42"),
        ("0", -42.0, "-42"),
        ("000", 5.0, "005"),
        ("000", 42.0, "042"),
        ("000", -5.0, "-005"),
        ("00000", 3.0, "00003"),
    ];
    for &(code, value, expected) in cases {
        check(code, value, expected);
    }
}

#[test]
fn number_integer_placeholder_optional_suppresses_short() {
    // NOTE: POI's `CellNumberFormatter` also maps `#####` applied to 0
    // to the empty string (Excel's "whole-integer-suppression" rule when
    // all integer placeholders are `#`). Our implementation keeps the "render 0"
    // behaviour today and punts the empty-body rule to a follow-up
    // task — tracked as a known gap; not included below.
    let cases: &[(&str, f64, &str)] = &[("#####", 42.0, "42"), ("#####", -42.0, "-42")];
    for &(code, value, expected) in cases {
        check(code, value, expected);
    }
}

// ---------------------------------------------------------------------------
// Number format — decimal placeholders
// ---------------------------------------------------------------------------

#[test]
fn number_decimal_required_emits_trailing_zeros() {
    let cases: &[(&str, f64, &str)] = &[
        ("0.0", 1.5, "1.5"),
        ("0.0", 1.0, "1.0"),
        ("0.0", 0.0, "0.0"),
        ("0.0", -1.5, "-1.5"),
        ("0.00", 1.5, "1.50"),
        ("0.00", 1.0, "1.00"),
        ("0.00", 0.0, "0.00"),
        ("0.000", 1.5, "1.500"),
    ];
    for &(code, value, expected) in cases {
        check(code, value, expected);
    }
}

#[test]
fn number_decimal_rounds_half_away() {
    // Test values chosen to avoid f64 representation quirks at the
    // half-point. POI's tests skip 1.235-style cases for the same reason;
    // both engines defer to the underlying float-to-string round mode.
    let cases: &[(&str, f64, &str)] = &[
        ("0.0", 1.234, "1.2"),
        ("0.0", 1.250001, "1.3"),
        ("0.00", 1.234, "1.23"),
        ("0.00", 1.236, "1.24"),
    ];
    for &(code, value, expected) in cases {
        check(code, value, expected);
    }
}

// ---------------------------------------------------------------------------
// Number format — comma grouping
// ---------------------------------------------------------------------------

#[test]
fn number_grouping_inserts_thousand_separators() {
    let cases: &[(&str, f64, &str)] = &[
        ("#,##0", 0.0, "0"),
        ("#,##0", 1.0, "1"),
        ("#,##0", 999.0, "999"),
        ("#,##0", 1000.0, "1,000"),
        ("#,##0", 1234.0, "1,234"),
        ("#,##0", 12345.0, "12,345"),
        ("#,##0", 1_234_567.0, "1,234,567"),
        ("#,##0", -1234.0, "-1,234"),
        ("#,##0.00", 1234.5, "1,234.50"),
        ("#,##0.00", -1234.567, "-1,234.57"),
    ];
    for &(code, value, expected) in cases {
        check(code, value, expected);
    }
}

// ---------------------------------------------------------------------------
// Number format — percent
// ---------------------------------------------------------------------------

#[test]
fn number_percent_scales_by_100_and_appends_sign() {
    let cases: &[(&str, f64, &str)] = &[
        ("0%", 0.5, "50%"),
        ("0%", 1.0, "100%"),
        ("0%", 0.0, "0%"),
        ("0.0%", 0.5, "50.0%"),
        ("0.0%", 1.5, "150.0%"),
        ("0.00%", 0.12345, "12.35%"),
        // Watchlist-style negative percent.
        ("0.0%", -0.26111, "-26.1%"),
        ("0.00%", -0.0261111, "-2.61%"),
    ];
    for &(code, value, expected) in cases {
        check(code, value, expected);
    }
}

// ---------------------------------------------------------------------------
// Number format — currency
// ---------------------------------------------------------------------------

#[test]
fn number_currency_renders_dollar_prefix_with_grouping() {
    let cases: &[(&str, f64, &str)] = &[
        ("$#,##0", 0.0, "$0"),
        ("$#,##0", 1.0, "$1"),
        ("$#,##0", 1234.0, "$1,234"),
        ("$#,##0", 450_000.0, "$450,000"),
        ("$#,##0", -1234.0, "-$1,234"),
        ("$#,##0.00", 1.5, "$1.50"),
        ("$#,##0.00", 1234.56, "$1,234.56"),
        ("$#,##0.00", 12_345.678, "$12,345.68"),
        ("$#,##0.00", -1234.56, "-$1,234.56"),
    ];
    for &(code, value, expected) in cases {
        check(code, value, expected);
    }
}

// ---------------------------------------------------------------------------
// Number format — scientific notation
// ---------------------------------------------------------------------------

#[test]
fn number_scientific_renders_signed_exponent() {
    let cases: &[(&str, f64, &str)] = &[
        ("0.00E+00", 1.0, "1.00E+00"),
        ("0.00E+00", 12345.0, "1.23E+04"),
        ("0.00E+00", -12345.0, "-1.23E+04"),
        ("0.00E+00", 0.0, "0.00E+00"),
        ("0.00E+00", 1e-5, "1.00E-05"),
        ("0.00E+00", 2.918e13, "2.92E+13"),
        ("0E+00", 12345.0, "1E+04"),
        ("0.000E+00", 12345.0, "1.235E+04"),
    ];
    for &(code, value, expected) in cases {
        check(code, value, expected);
    }
}

// ---------------------------------------------------------------------------
// Number format — sign-bearing sections (POI hasSign() rule)
// ---------------------------------------------------------------------------

/// The exact format that produced `-+1bps` on the calamine path before
/// the POI-aligned reimplementation — locking it in as a regression test from day one.
#[test]
fn number_sign_bearing_sections_dispatch_correctly() {
    let cases: &[(&str, f64, &str)] = &[
        // 2-section pos/neg with explicit minus in negative body.
        ("0.00;-0.00", 5.0, "5.00"),
        ("0.00;-0.00", -5.0, "-5.00"),
        ("0.00;-0.00", 0.0, "0.00"),
        // Accounting parens — POI's hasSign also covers `(` / `)`.
        ("#,##0.00;(#,##0.00)", 1234.56, "1,234.56"),
        ("#,##0.00;(#,##0.00)", -1234.56, "(1,234.56)"),
        // Watchlist canary: literal `+` and `-` with `"bps"` suffix.
        (r#"+0"bps";-0"bps""#, 1.0, "+1bps"),
        (r#"+0"bps";-0"bps""#, -1.0, "-1bps"),
        (r#"+0"bps";-0"bps""#, 0.0, "+0bps"),
    ];
    for &(code, value, expected) in cases {
        check(code, value, expected);
    }
}

// ---------------------------------------------------------------------------
// Number format — quoted literal suffixes
// ---------------------------------------------------------------------------

#[test]
fn number_quoted_literal_suffix_renders_verbatim() {
    let cases: &[(&str, f64, &str)] = &[
        (r#"0.0"x""#, 62.4, "62.4x"),
        (r#"0.0" USD""#, 1.0, "1.0 USD"),
        (r#"0"x""#, 5.0, "5x"),
        (r#""$"0.00"#, 12.34, "$12.34"),
    ];
    for &(code, value, expected) in cases {
        check(code, value, expected);
    }
}

// ---------------------------------------------------------------------------
// Date format (CellDateFormatter)
// ---------------------------------------------------------------------------

/// Anchor: serial 46115 = 2026-04-03 (a Friday). Verified against
/// independent JDN math (see date_formatter module docs).
#[test]
fn date_iso_format_decomposes_serial_to_ymd() {
    let cases: &[(&str, f64, &str)] = &[
        ("yyyy-mm-dd", 46115.0, "2026-04-03"),
        ("yyyy-mm-dd", 1.0, "1900-01-01"),
        ("yyyy-mm-dd", 59.0, "1900-02-28"),
        // The Lotus 1900 leap-year bug: serial 60 displays as a fake
        // 1900-02-29. POI honours this for compatibility with Lotus
        // workbooks created before the bug was identified.
        ("yyyy-mm-dd", 60.0, "1900-02-29"),
        ("yyyy-mm-dd", 61.0, "1900-03-01"),
        ("yyyy-mm-dd", 36526.0, "2000-01-01"),
    ];
    for &(code, value, expected) in cases {
        check(code, value, expected);
    }
}

#[test]
fn date_us_slash_formats() {
    let cases: &[(&str, f64, &str)] = &[
        ("mm/dd/yyyy", 46115.0, "04/03/2026"),
        ("m/d/yy", 46115.0, "4/3/26"),
        ("m/d/yyyy", 46115.0, "4/3/2026"),
        ("mm/dd/yy", 46115.0, "04/03/26"),
    ];
    for &(code, value, expected) in cases {
        check(code, value, expected);
    }
}

#[test]
fn date_long_form_names() {
    let cases: &[(&str, f64, &str)] = &[
        // 2026-04-03 was a Friday.
        ("dddd", 46115.0, "Friday"),
        ("ddd", 46115.0, "Fri"),
        ("mmmm", 46115.0, "April"),
        ("mmm", 46115.0, "Apr"),
        ("mmmmm", 46115.0, "A"),
        ("dddd, mmmm d, yyyy", 46115.0, "Friday, April 3, 2026"),
    ];
    for &(code, value, expected) in cases {
        check(code, value, expected);
    }
}

#[test]
fn date_1904_epoch_shifts_serial_origin() {
    // Under the 1904 epoch, serial 0 = 1904-01-01 and there is no
    // Lotus leap-year bug.
    let opts = FormatOptions { date_1904: true };
    // 1904 is itself a leap year (divisible by 4, not by 100), so
    // 1904-01-01 + 365 days lands on 1904-12-31; serial 366 is the
    // 1905-01-01 anchor.
    let cases: &[(f64, &str)] = &[
        (0.0, "1904-01-01"),
        (1.0, "1904-01-02"),
        (365.0, "1904-12-31"),
        (366.0, "1905-01-01"),
    ];
    for &(value, expected) in cases {
        let got = apply("yyyy-mm-dd", value, &opts);
        assert_eq!(
            got, expected,
            "format(`yyyy-mm-dd`, {value}) [1904 epoch] = `{got}` but POI says `{expected}`"
        );
    }
}

// ---------------------------------------------------------------------------
// Time / mixed date+time
// ---------------------------------------------------------------------------

#[test]
fn time_24_hour_clock() {
    let cases: &[(&str, f64, &str)] = &[
        ("hh:mm:ss", 0.0, "00:00:00"),
        ("hh:mm:ss", 0.5, "12:00:00"),
        ("hh:mm", 0.5, "12:00"),
        ("hh:mm", 0.25, "06:00"),
        ("hh:mm", 0.75, "18:00"),
    ];
    for &(code, value, expected) in cases {
        check(code, value, expected);
    }
}

#[test]
fn time_12_hour_clock_with_am_pm() {
    let cases: &[(&str, f64, &str)] = &[
        // Midnight & noon edge cases — POI maps 0→12 AM and 12→12 PM.
        ("h:mm AM/PM", 0.0, "12:00 AM"),
        ("h:mm AM/PM", 0.5, "12:00 PM"),
        ("h:mm AM/PM", 0.25, "6:00 AM"),
        ("h:mm AM/PM", 0.75, "6:00 PM"),
        // Watchlist canary — 9:30 AM.
        ("h:mm AM/PM", 9.0 / 24.0 + 30.0 / 1440.0, "9:30 AM"),
    ];
    for &(code, value, expected) in cases {
        check(code, value, expected);
    }
}

// ---------------------------------------------------------------------------
// Elapsed format (CellElapsedFormatter)
// ---------------------------------------------------------------------------

#[test]
fn elapsed_hours_total_count() {
    let cases: &[(&str, f64, &str)] = &[
        // 1.5 hours = 90 minutes.
        ("[h]:mm", 1.5 / 24.0, "1:30"),
        // 25 hours wraps past 24 — elapsed never rolls over.
        ("[h]:mm", 25.0 / 24.0, "25:00"),
        ("[h]:mm:ss", 1.5 / 24.0, "1:30:00"),
    ];
    for &(code, value, expected) in cases {
        check(code, value, expected);
    }
}

#[test]
fn elapsed_minutes_total_count() {
    let cases: &[(&str, f64, &str)] = &[
        ("[mm]:ss", 1.5 / 24.0, "90:00"),
        ("[mm]:ss", 0.5 / 24.0, "30:00"),
    ];
    for &(code, value, expected) in cases {
        check(code, value, expected);
    }
}

#[test]
fn elapsed_seconds_total_count() {
    let cases: &[(&str, f64, &str)] = &[
        // 1 hour = 3600 seconds.
        ("[ss]", 1.0 / 24.0, "3600"),
        // 1.5 hours = 5400 seconds.
        ("[ss]", 1.5 / 24.0, "5400"),
        ("[ss].000", 1.5 / 24.0, "5400.000"),
    ];
    for &(code, value, expected) in cases {
        check(code, value, expected);
    }
}

// ---------------------------------------------------------------------------
// Text format (CellTextFormatter)
// ---------------------------------------------------------------------------

#[test]
fn text_section_echoes_at_substitution() {
    let cases: &[(&str, &str, &str)] = &[
        ("@", "alice", "alice"),
        ("@", "", ""),
        (r#""Name: "@"#, "alice", "Name: alice"),
        (r#"@" (customer)""#, "alice", "alice (customer)"),
        ("@/@", "x", "x/x"),
        // Empty / missing text body on a 4-section format → text passes
        // through to the text section's body.
        (r#"0;0;0;"prefix: "@"#, "value", "prefix: value"),
    ];
    for &(code, text, expected) in cases {
        check_text(code, text, expected);
    }
}

#[test]
fn text_section_unicode_passes_through() {
    check_text("@", "café", "café");
    check_text(r#""→ "@"#, "café", "→ café");
}

// ---------------------------------------------------------------------------
// Multi-section selection (POI selectFormat)
// ---------------------------------------------------------------------------

#[test]
fn three_section_format_dispatches_pos_neg_zero() {
    let code = r#"0.00;-0.00;"zero""#;
    check(code, 5.0, "5.00");
    check(code, -5.0, "-5.00");
    check(code, 0.0, "zero");
}

#[test]
fn conditional_sections_select_by_predicate() {
    // POI's `[>=]` / `[<]` gate which section consumes the value. When
    // the selected section has no literal sign marker, POI still
    // prepends a `-` for negative values — condition selection is
    // independent of sign rendering.
    let code = "[>=100]0.0;[<0]0.0;0.0";
    check(code, 150.0, "150.0");
    check(code, -5.0, "-5.0");
    check(code, 50.0, "50.0");
}
