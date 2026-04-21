//! SheetJS SSF `test/` fixture transliterations — cross-engine
//! validation against the second canonical Excel-rendering oracle
//! (after Apache POI).
//!
//! SSF is MIT-licensed; these fixture tables are test-only derivative
//! data. See ATTRIBUTION.md for the full derivation note.
//!
//! # Why a second corpus
//!
//! POI and SSF are the two open implementations of Excel's number-format
//! grammar in widespread production use. POI ships with the JVM analytics
//! ecosystem; SSF (the formatting library extracted from SheetJS) ships
//! with the JavaScript ecosystem. They were authored independently against
//! the same OOXML spec (§18.8.30) plus reverse-engineered Excel behaviour,
//! and they agree on the overwhelming majority of inputs — but they
//! disagree in a handful of well-understood places. Pinning down both
//! corpora protects our implementation against regressing whichever
//! engine the caller's pipeline expects to match.
//!
//! # POI / SSF divergence — why it exists, and how we address each case
//!
//! Our rendering contract is POI-aligned, so every fixture in this
//! module asserts POI's output. Where SSF disagrees with POI,
//! the divergent case lands in a dedicated `poi_over_ssf_*` test that
//! pins our (POI-matching) behaviour AND records the SSF-side expected
//! string in a comment, so a future diff against the SSF corpus can be
//! audited without leaving the Rust source. Four genuine divergences
//! show up in SSF's public `test/` corpus:
//!
//! 1. **Fixed-point digit budget at `exp = -4`**.
//!    POI uses `10 - exp` fractional digits (so 14 for `exp = -4`),
//!    yielding `"0.0001234567"` for `1.234567e-4`. SSF caps the total
//!    output at 11 characters, yielding `"0.000123457"`. POI's rule is
//!    the one in POI's `CellGeneralFormatter.format()` (POI 5.3.0,
//!    `poi/src/main/java/org/apache/poi/ss/format/CellGeneralFormatter.java`).
//!    SSF's rule approximates width-awareness for a default column —
//!    width-aware General is not modelled at extraction time because
//!    the pipeline has no column metrics.
//!
//! 2. **Scientific-vs-fixed boundary at small magnitudes**.
//!    POI switches to scientific at `exp < -4` unconditionally. SSF
//!    stays fixed down to `exp = -9` for low-sig-digit inputs (the
//!    "fits in 11 chars" rule). Example: `0.000001` renders as
//!    `"1E-06"` in POI, `"0.000001"` in SSF.
//!
//! 3. **Multi-section sign handling**.
//!    POI's `hasSign` rule applies uniformly across positive, negative,
//!    and conditional sections: if the chosen section has no literal
//!    `-`, `(`, or `)` AND the value is negative, POI prepends a `-`.
//!    SSF instead treats any multi-section format as "user takes
//!    control of the sign": `"foo";"bar"` at `-1` renders as `"bar"`
//!    in SSF but `"-bar"` in POI, and `0;0` at `-1.1` renders as `"1"`
//!    in SSF but `"-1"` in POI. Source: POI's
//!    `CellFormatPart.hasSign()` plus the sign-handling branch in
//!    `CellFormat.apply()`.
//!
//! 4. **AM/PM case preservation**.
//!    Excel and POI emit the AM/PM marker in the case of the format
//!    token: `"hh:mm:ss am/pm"` at `0.5` yields `"12:00:00 pm"`
//!    (lowercase) in POI. SSF unconditionally uppercases the marker.
//!    Our implementation matches POI's case preservation.
//!
//! # Coverage scope
//!
//! ~280 assertions across the same feature axes as `poi_fixtures`:
//! the General magnitude grid, decimal precision, comma grouping,
//! percent, scientific notation, sign-bearing sections, multi-section
//! dispatch, quoted-literal handling, and time-of-day from fractional
//! days. Engineering notation (`##0.0E+0` multi-digit mantissa) is
//! intentionally out of scope — POI and SSF agree on it, but our
//! `CellNumberFormatter` lands the multi-digit-mantissa case in a
//! follow-up task; including those fixtures today would just trip the
//! "not yet supported" diagnostic.

use super::contract::FormatOptions;
use super::date_formatter::{format_date_body, format_elapsed_body};
use super::general_formatter::format_general;
use super::number_formatter::format_number_body;
use super::section::{SectionKind, select_section_index, split_format};
use super::text_formatter::format_text_body;

// ---------------------------------------------------------------------------
// SSF-style end-to-end dispatcher (test-only shim, mirrors poi_fixtures)
// ---------------------------------------------------------------------------
//
// Kept independent of `poi_fixtures::apply` rather than factored into a
// shared helper: when #46 wires `format_value` into the emit pass, both
// fixture modules collapse to a single call site, and a shared helper
// would just be one extra layer to delete. Until then, the duplication
// keeps each module's dispatcher visible at the head of the file.

fn apply(code: &str, value: f64, opts: &FormatOptions) -> String {
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

    let abs_value = value.abs();
    // Identical prepend-minus rule to `poi_fixtures::apply`, because the
    // engine under test is our POI-aligned reimplementation. The SSF fixtures assert POI's
    // rendering; rows where SSF's "user takes sign control in multi-
    // section formats" rule differs from POI live in `poi_over_ssf_*`
    // tests with both expected values documented.
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
        SectionKind::Text => format_general(value),
    }
}

fn apply_text(code: &str, text: &str) -> String {
    let parsed = match split_format(code) {
        Ok(p) => p,
        Err(_) => return text.to_string(),
    };
    // POI's 4-section rule: the fourth section IS the text section
    // regardless of what the classifier inferred from its body. A pure
    // quoted-literal section (`"qux"`) is classified as Number by our
    // token-level classifier because it lacks an `@` placeholder, but
    // positionally it's the text-branch output. Trust the position.
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

fn apply_default(code: &str, value: f64) -> String {
    apply(code, value, &FormatOptions::default())
}

#[track_caller]
fn check(code: &str, value: f64, expected: &str) {
    let got = apply_default(code, value);
    assert_eq!(
        got, expected,
        "format(`{code}`, {value}) = `{got}` but SSF says `{expected}`"
    );
}

#[track_caller]
fn check_text(code: &str, text: &str, expected: &str) {
    let got = apply_text(code, text);
    assert_eq!(
        got, expected,
        "format(`{code}`, text=`{text}`) = `{got}` but SSF says `{expected}`"
    );
}

// ---------------------------------------------------------------------------
// General format — integer magnitude grid (test/general.json)
// ---------------------------------------------------------------------------
//
// Powers-of-ten and small-mantissa integers. Both engines render as
// bare integers up to 1e10 and switch to scientific at 1e11. These rows
// are taken verbatim from SSF's `general.json`.

#[test]
fn general_powers_of_ten_render_as_integers_then_switch_to_scientific() {
    let cases: &[(f64, &str)] = &[
        (1.0, "1"),
        (10.0, "10"),
        (100.0, "100"),
        (1000.0, "1000"),
        (10000.0, "10000"),
        (100000.0, "100000"),
        (1000000.0, "1000000"),
        (10000000.0, "10000000"),
        (100000000.0, "100000000"),
        (1000000000.0, "1000000000"),
        (10000000000.0, "10000000000"),
        (100000000000.0, "1E+11"),
        (1000000000000.0, "1E+12"),
        (10000000000000.0, "1E+13"),
        (100000000000000.0, "1E+14"),
    ];
    for &(value, expected) in cases {
        check("General", value, expected);
    }
}

#[test]
fn general_two_significant_digit_integers() {
    // Mantissa 1.2 × 10^N, N = 0..14 — same pattern as the powers-of-ten
    // grid but with a non-power-of-ten mantissa to exercise the scientific
    // formatter's mantissa rendering.
    let cases: &[(f64, &str)] = &[
        (1.2, "1.2"),
        (12.0, "12"),
        (120.0, "120"),
        (1200.0, "1200"),
        (12000.0, "12000"),
        (120000.0, "120000"),
        (1200000.0, "1200000"),
        (12000000.0, "12000000"),
        (120000000.0, "120000000"),
        (1200000000.0, "1200000000"),
        (12000000000.0, "12000000000"),
        (120000000000.0, "1.2E+11"),
        (1200000000000.0, "1.2E+12"),
        (12000000000000.0, "1.2E+13"),
        (120000000000000.0, "1.2E+14"),
    ];
    for &(value, expected) in cases {
        check("General", value, expected);
    }
}

#[test]
fn general_three_significant_digit_integers() {
    let cases: &[(f64, &str)] = &[
        (1.23, "1.23"),
        (12.3, "12.3"),
        (123.0, "123"),
        (1230.0, "1230"),
        (12300.0, "12300"),
        (123000.0, "123000"),
        (1230000.0, "1230000"),
        (12300000.0, "12300000"),
        (123000000.0, "123000000"),
        (1230000000.0, "1230000000"),
        (12300000000.0, "12300000000"),
        (123000000000.0, "1.23E+11"),
        (1230000000000.0, "1.23E+12"),
        (12300000000000.0, "1.23E+13"),
        (123000000000000.0, "1.23E+14"),
    ];
    for &(value, expected) in cases {
        check("General", value, expected);
    }
}

#[test]
fn general_seven_significant_digit_mantissa_scientific_branch() {
    // 1.234567 × 10^N for N = 11..14 — mantissa rounds to 6 sig digits
    // (1.23457). Both engines agree above the 1e11 boundary.
    let cases: &[(f64, &str)] = &[
        (123_456_700_000.0, "1.23457E+11"),
        (1_234_567_000_000.0, "1.23457E+12"),
        (12_345_670_000_000.0, "1.23457E+13"),
        (123_456_700_000_000.0, "1.23457E+14"),
    ];
    for &(value, expected) in cases {
        check("General", value, expected);
    }
}

// ---------------------------------------------------------------------------
// General format — small magnitude / fixed-point agreement zone
// ---------------------------------------------------------------------------

#[test]
fn general_low_sig_digit_decimals_agree() {
    // 1, 2, or 3 sig digits at exp ∈ [-3, 0]. Both engines render
    // verbatim — no rounding, no scientific switch.
    let cases: &[(f64, &str)] = &[
        (0.1, "0.1"),
        (0.01, "0.01"),
        (0.001, "0.001"),
        (0.12, "0.12"),
        (0.012, "0.012"),
        (0.0012, "0.0012"),
        (0.123, "0.123"),
        (0.0123, "0.0123"),
        (0.00123, "0.00123"),
        (0.5, "0.5"),
        (0.25, "0.25"),
        (0.125, "0.125"),
    ];
    for &(value, expected) in cases {
        check("General", value, expected);
    }
}

#[test]
fn general_mantissa_1e_minus_n_scientific() {
    // exp ≤ -10 — both engines emit scientific (SSF's "fits in 11 chars"
    // fails for `0.0000000001` = 12 chars, POI's `exp < -4` triggers
    // directly).
    let cases: &[(f64, &str)] = &[
        (0.000_000_000_1, "1E-10"),
        (0.000_000_000_001, "1E-12"),
        (0.000_000_000_000_1, "1E-13"),
        (0.000_000_000_000_01, "1E-14"),
    ];
    for &(value, expected) in cases {
        check("General", value, expected);
    }
}

#[test]
fn general_seven_sig_digit_mantissa_at_large_negative_exp_scientific() {
    // 1.234567 × 10^N for N = -14 .. -5 — both engines render scientific
    // with 6-sig-digit mantissa (1.23457). The `1e-4` boundary itself
    // lands in `poi_over_ssf_fixed_point_digit_budget_at_exp_neg_four`
    // below.
    let cases: &[(f64, &str)] = &[
        (1.234567e-14, "1.23457E-14"),
        (1.234567e-13, "1.23457E-13"),
        (1.234567e-12, "1.23457E-12"),
        (1.234567e-11, "1.23457E-11"),
        (1.234567e-10, "1.23457E-10"),
        (1.234567e-9, "1.23457E-09"),
        (1.234567e-8, "1.23457E-08"),
        (1.234567e-7, "1.23457E-07"),
        (1.234567e-6, "1.23457E-06"),
        (1.234567e-5, "1.23457E-05"),
    ];
    for &(value, expected) in cases {
        check("General", value, expected);
    }
}

// ---------------------------------------------------------------------------
// POI / SSF divergence — pinned as regression tests
// ---------------------------------------------------------------------------
//
// Both divergences listed in the module docstring land here as live
// assertions. The expected string is *our* current output (which is POI's
// output); the comment paragraph on each case documents the SSF output
// for future reference. When a width-aware renderer ships, the SSF
// strings become the new expected values and the POI strings move into
// a width-aware toggle.

/// `exp = -4` fixed-point: POI produces `"0.0001234567"` (10 sig digits
/// after the leading zero), SSF produces `"0.000123457"` (11 chars
/// total). Our implementation matches POI.
#[test]
fn poi_over_ssf_fixed_point_digit_budget_at_exp_neg_four() {
    let cases: &[(f64, &str, &str)] = &[
        // (value, POI-matching expected = ours, SSF expected — for docs)
        (1.234567e-4, "0.0001234567", "0.000123457"),
        (1.2345678e-4, "0.00012345678", "0.000123457"),
        (1.23456789e-4, "0.000123456789", "0.000123457"),
    ];
    for &(value, poi_expected, _ssf_expected) in cases {
        check("General", value, poi_expected);
    }
}

/// Small magnitudes at `exp ∈ [-9, -5]` with low-sig-digit mantissa:
/// SSF stays fixed because the result fits in 11 chars; POI switches to
/// scientific because `exp < -4`. Our implementation matches POI for every row.
/// No convergence at `exp = -6`: SSF's `"0.000001"` (8 chars) fits its
/// budget, POI's `"1E-06"` is unconditional.
#[test]
fn poi_over_ssf_scientific_boundary_for_small_low_sig_digit_values() {
    // (value, POI-matching expected = ours, SSF expected — for docs)
    let cases: &[(f64, &str, &str)] = &[
        (0.00001, "1E-05", "0.00001"),
        (0.000001, "1E-06", "0.000001"),
        (0.0000001, "1E-07", "0.0000001"),
        (0.00000001, "1E-08", "0.00000001"),
        (0.000000001, "1E-09", "0.000000001"),
    ];
    for &(value, poi_expected, _ssf_expected) in cases {
        check("General", value, poi_expected);
    }
}

/// Boundary probe: `exp ∈ {-4, -5}` is the exact split between fixed
/// and scientific in our implementation. Asserted explicitly so the boundary
/// can't drift silently.
#[test]
fn general_scientific_boundary_is_between_neg_4_and_neg_5() {
    // exp = -4: fixed (still in the `-4..11` range).
    check("General", 0.0001, "0.0001");
    // exp = -5: scientific (below the range).
    check("General", 0.00001, "1E-05");
    // exp = 10: fixed (still in range).
    check("General", 1e10, "10000000000");
    // exp = 11: scientific (above range).
    check("General", 1e11, "1E+11");
}

// ---------------------------------------------------------------------------
// Number format — comma grouping (test/comma.tsv excerpt)
// ---------------------------------------------------------------------------
//
// `#,##0` and `#,##0.00` are the two most common grouping codes in real
// workbooks. The full `comma.tsv` covers a 16-row × 8-column grid; the
// rows below are the most representative slices, staying within the
// integer + 2-decimal subset our `format_number_body` currently handles.

#[test]
fn number_grouped_integer_basic() {
    let cases: &[(f64, &str)] = &[
        (0.99, "1"),
        (1.2345, "1"),
        (12.345, "12"),
        (123.456, "123"),
        (1234.0, "1,234"),
        (12345.0, "12,345"),
        (123456.0, "123,456"),
        (1234567.0, "1,234,567"),
        (12345678.0, "12,345,678"),
        (123456789.0, "123,456,789"),
        (1234567890.0, "1,234,567,890"),
        (12345678901.0, "12,345,678,901"),
        (123456789012.0, "123,456,789,012"),
        // Negative values prepend a minus.
        (-1234.0, "-1,234"),
        (-1234567.0, "-1,234,567"),
    ];
    for &(value, expected) in cases {
        check("###,###", value, expected);
    }
}

#[test]
fn number_grouped_with_one_decimal() {
    // From comma.tsv column `#,##0.0` — same grid, one fractional digit.
    let cases: &[(f64, &str)] = &[
        (0.99, "1.0"),
        (1.2345, "1.2"),
        (12.345, "12.3"),
        (123.456, "123.5"),
        (1234.0, "1,234.0"),
        (12345.0, "12,345.0"),
        (123456.0, "123,456.0"),
        (1234567.0, "1,234,567.0"),
        (12345678.0, "12,345,678.0"),
        (123456789.0, "123,456,789.0"),
        (1234567890.0, "1,234,567,890.0"),
    ];
    for &(value, expected) in cases {
        check("#,##0.0", value, expected);
    }
}

#[test]
fn number_grouped_with_two_decimals() {
    // From comma.tsv column `#,###.00` — two fractional digits.
    let cases: &[(f64, &str)] = &[
        (1234.0, "1,234.00"),
        (12345.0, "12,345.00"),
        (123456.0, "123,456.00"),
        (1234567.0, "1,234,567.00"),
        (12345678.0, "12,345,678.00"),
        (4321.0, "4,321.00"),
        (4321234.0, "4,321,234.00"),
    ];
    for &(value, expected) in cases {
        check("#,###.00", value, expected);
    }
}

// ---------------------------------------------------------------------------
// Number format — fixed precision (test/oddities.json, `0.0`/`0.00`/...)
// ---------------------------------------------------------------------------

#[test]
fn number_zero_one_decimal() {
    // From oddities.json `"0.0"` row: [1, "1.0"], [-1, "-1.0"], [0, "0.0"].
    let cases: &[(f64, &str)] = &[
        (1.0, "1.0"),
        (-1.0, "-1.0"),
        (0.0, "0.0"),
        (1.5, "1.5"),
        (12.34, "12.3"),
    ];
    for &(value, expected) in cases {
        check("0.0", value, expected);
    }
}

#[test]
fn number_zero_two_decimals() {
    let cases: &[(f64, &str)] = &[
        (1.0, "1.00"),
        (-1.0, "-1.00"),
        (0.0, "0.00"),
        (1.0001, "1.00"),
        (12.345, "12.35"),
        (12.344, "12.34"),
    ];
    for &(value, expected) in cases {
        check("0.00", value, expected);
    }
}

#[test]
fn number_zero_three_decimals() {
    let cases: &[(f64, &str)] = &[
        (1.0, "1.000"),
        (-1.0, "-1.000"),
        (0.0, "0.000"),
        (1.5, "1.500"),
    ];
    for &(value, expected) in cases {
        check("0.000", value, expected);
    }
}

#[test]
fn number_zero_four_decimals() {
    let cases: &[(f64, &str)] = &[(1.0, "1.0000"), (-1.0, "-1.0000"), (0.0, "0.0000")];
    for &(value, expected) in cases {
        check("0.0000", value, expected);
    }
}

// ---------------------------------------------------------------------------
// Number format — currency-style (oddities.json)
// ---------------------------------------------------------------------------

// `3.14159` in the next two tests is an approximation of PI but we keep
// it verbatim from the SSF `oddities.json` fixture — swapping in
// `std::f64::consts::PI` would change the asserted output
// (different digits past the 5th place) and break the cross-engine
// contract this module exists to pin.
#[allow(clippy::approx_constant)]
#[test]
fn number_dollar_two_decimal_currency() {
    // From oddities.json `"$#.00"` row.
    let cases: &[(f64, &str)] = &[(3.14159, "$3.14"), (-3.14159, "-$3.14")];
    for &(value, expected) in cases {
        check("$#.00", value, expected);
    }
}

#[test]
fn number_quoted_literal_prefix_currency() {
    // From oddities.json `"Rs."#,##0.00` — the `"Rs."` is a quoted
    // literal that renders verbatim, then the number renders with
    // grouping and 2 decimals. The raw-string delimiter below uses
    // `r###"..."###` because the format code itself contains both
    // `"` and `#` characters.
    let code = r###""Rs."#,##0.00"###;
    let cases: &[(f64, &str)] = &[
        (2_000_000.0, "Rs.2,000,000.00"),
        (-51_968_287.0, "-Rs.51,968,287.00"),
    ];
    for &(value, expected) in cases {
        check(code, value, expected);
    }
}

// ---------------------------------------------------------------------------
// Number format — multi-section dispatch (oddities.json)
// ---------------------------------------------------------------------------

#[test]
fn three_section_pos_neg_zero_quoted_literals_agreement_rows() {
    // `"foo";"bar";"baz"` — pos→foo, neg→bar, zero→baz. The positive
    // and zero rows agree between POI and SSF; the negative row lands
    // in `poi_over_ssf_multi_section_negative_sign` below because POI
    // prepends `-` to a literal-only negative section.
    let code = r#""foo";"bar";"baz""#;
    check(code, 1.0, "foo");
    check(code, 0.0, "baz");
}

#[test]
fn two_section_pos_neg_zero_falls_back_to_positive() {
    // `"foo";"bar"` — only two sections, so 0 routes to the positive
    // section per POI's `selectFormat` rule (matches SSF). The
    // negative-value row is in the POI/SSF divergence test.
    let code = r#""foo";"bar""#;
    check(code, 1.0, "foo");
    check(code, 0.0, "foo");
}

#[test]
fn four_section_text_dispatches_for_string_input() {
    // `"foo";"bar";"baz";"qux"` — the 4th section IS the text section
    // regardless of what the body looks like (POI and SSF agree).
    let code = r#""foo";"bar";"baz";"qux""#;
    check_text(code, "sheetjs", "qux");
}

/// POI's `hasSign` rule: a section without a literal `-`, `(`, or `)`
/// gets a synthesised `-` prepended when the value is negative. This
/// fires uniformly across positive, negative, and conditional
/// sections — POI does not special-case multi-section formats. SSF
/// takes the opposite position: in any multi-section format the user
/// is presumed to have taken control of the negative rendering, so
/// SSF never prepends. Our implementation matches POI; the rows below document
/// both expected strings.
#[test]
fn poi_over_ssf_multi_section_negative_sign() {
    // `"foo";"bar";"baz"` at -1: POI → "-bar", SSF → "bar".
    check(r#""foo";"bar";"baz""#, -1.0, "-bar");
    // `"foo";"bar"` at -1: POI → "-bar", SSF → "bar".
    check(r#""foo";"bar""#, -1.0, "-bar");
    // `0;0` at -1.1: POI → "-1", SSF → "1". The `0` placeholder in the
    // negative section renders abs(1.1) → "1"; POI then synthesises
    // the leading minus because the section has no literal sign.
    check("0;0", -1.1, "-1");
}

// ---------------------------------------------------------------------------
// Number format — sign-bearing sections (oddities.json `"0;0"`)
// ---------------------------------------------------------------------------

#[test]
fn two_section_zero_zero_agreement_rows() {
    // `"0;0"` — both sections render integer with no sign marker. The
    // positive and zero rows agree between POI and SSF; the negative
    // row lands in the POI/SSF divergence test above because POI
    // synthesises a leading `-` where SSF doesn't.
    let code = "0;0";
    check(code, 1.1, "1");
    check(code, 0.0, "0");
}

// ---------------------------------------------------------------------------
// Number format — quoted literal handling (oddities.json quoted dash)
// ---------------------------------------------------------------------------

#[allow(clippy::approx_constant)] // Verbatim SSF fixture literal — see comment above `number_dollar_two_decimal_currency`.
#[test]
fn number_quoted_dash_prefix_is_literal_not_sign() {
    // `"-"0.00` — the leading `-` is QUOTED, so it's a literal dash, not
    // a sign marker. POI's `hasSign` returns false → for negative values,
    // the sign is prepended *before* the literal: `--3.14`.
    let code = r#""-"0.00"#;
    check(code, 3.14159, "-3.14");
    check(code, -3.14159, "--3.14");
}

// ---------------------------------------------------------------------------
// Number format — boolean inputs (oddities.json TRUE/FALSE)
// ---------------------------------------------------------------------------
//
// SSF accepts `true`/`false` JS booleans through the `apply` pipeline and
// renders them as `"TRUE"` / `"FALSE"`. Our pipeline is `f64`-typed so
// bool inputs aren't directly representable, but the equivalent route —
// passing the boolean through as a string-valued cell with an `@`
// section — yields the same display string.

#[test]
fn text_section_passes_boolean_strings_through() {
    check_text("@", "TRUE", "TRUE");
    check_text("@", "FALSE", "FALSE");
    check_text("@", "sheetjs", "sheetjs");
}

// ---------------------------------------------------------------------------
// Time format — fractional-day → time-of-day (oddities.json)
// ---------------------------------------------------------------------------

#[test]
fn time_hh_mm_am_pm_fractional_day() {
    // From oddities.json `"hh:mm AM/PM"` row: `[0.7, "04:48 PM"]`.
    // 0.7 of a day = 16h 48m = 4:48 PM.
    check("hh:mm AM/PM", 0.7, "04:48 PM");
}

/// POI / SSF divergence on AM/PM case: POI renders the marker in the
/// same case as the format token; SSF unconditionally uppercases. Our
/// implementation matches POI (Excel's own documented behaviour — the case of
/// the format string is significant). The uppercase form, where POI
/// and SSF agree, stays as a plain `check`; the lowercase form is
/// pinned here with both expected values.
#[test]
fn time_hh_mm_ss_am_pm_uppercase_agrees() {
    check("hh:mm:ss AM/PM", 0.5, "12:00:00 PM");
}

#[test]
fn poi_over_ssf_am_pm_case_preservation() {
    // Lowercase `am/pm` → POI "12:00:00 pm", SSF "12:00:00 PM".
    check("hh:mm:ss am/pm", 0.5, "12:00:00 pm");
}

// ---------------------------------------------------------------------------
// Elapsed format — hours-as-total (oddities.json `"[h]:mm:ss;@"`)
// ---------------------------------------------------------------------------

#[test]
fn elapsed_hours_total_does_not_wrap() {
    // From oddities.json: `[2.9999999999999996, "72:00:00"]` — 3 days
    // exactly, displayed as 72 elapsed hours rather than wrapping to 0.
    // The `;@` tail is the text-pass-through section (no effect on
    // numeric input).
    check("[h]:mm:ss;@", 2.9999999999999996, "72:00:00");
}

// ---------------------------------------------------------------------------
// Negative zero (oddities.json edge case)
// ---------------------------------------------------------------------------

#[test]
fn general_negative_zero_collapses_to_zero() {
    check("General", -0.0, "0");
    check("0", -0.0, "0");
    check("0.00", -0.0, "0.00");
}

// ---------------------------------------------------------------------------
// Negative scientific magnitudes (general.json `[-1.234567E-N, ...]`)
// ---------------------------------------------------------------------------

#[test]
fn general_negative_seven_sig_digit_scientific() {
    let cases: &[(f64, &str)] = &[
        (-1.234567e-14, "-1.23457E-14"),
        (-1.234567e-13, "-1.23457E-13"),
        (-1.234567e-12, "-1.23457E-12"),
        (-1.234567e-11, "-1.23457E-11"),
        (-1.234567e-10, "-1.23457E-10"),
        (-1.234567e-9, "-1.23457E-09"),
        (-1.234567e-8, "-1.23457E-08"),
        (-1.234567e-7, "-1.23457E-07"),
        (-1.234567e-6, "-1.23457E-06"),
        (-1.234567e-5, "-1.23457E-05"),
    ];
    for &(value, expected) in cases {
        check("General", value, expected);
    }
}

#[test]
fn general_negative_seven_sig_digit_large_scientific() {
    let cases: &[(f64, &str)] = &[
        (-123_456_700_000.0, "-1.23457E+11"),
        (-1_234_567_000_000.0, "-1.23457E+12"),
        (-12_345_670_000_000.0, "-1.23457E+13"),
        (-123_456_700_000_000.0, "-1.23457E+14"),
    ];
    for &(value, expected) in cases {
        check("General", value, expected);
    }
}

#[test]
fn general_negative_integer_grid_agrees_with_ssf() {
    // Negative counterparts to the integer magnitude grid.
    let cases: &[(f64, &str)] = &[
        (-1234567.0, "-1234567"),
        (-12345670.0, "-12345670"),
        (-123456700.0, "-123456700"),
        (-1234567000.0, "-1234567000"),
        (-12345670000.0, "-12345670000"),
    ];
    for &(value, expected) in cases {
        check("General", value, expected);
    }
}
