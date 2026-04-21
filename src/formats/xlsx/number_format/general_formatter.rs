//! POI-aligned CellGeneralFormatter — renders an `f64` against the
//! Excel "General" pseudo-format.
//!
//! # Why "General" needs its own formatter
//!
//! Excel's General format is not a number-format string — it's a
//! width-aware adaptive renderer. With unlimited width its rules are:
//!
//! 1. **Zero** → `"0"` (no decimal, no sign).
//! 2. **Integer-valued and `|v| < 1e15`** → integer rendering, no
//!    decimal point.
//! 3. **`|v| >= 1e11` or `|v| < 1e-4` (non-zero)** → scientific notation
//!    with up to 6 significant digits, exponent `E±NN` (always signed,
//!    minimum two digits).
//! 4. **Otherwise** → fixed-point with up to 11 significant digits
//!    total, trailing zeros trimmed and a dangling decimal point
//!    suppressed.
//!
//! Width-aware behaviour (Excel auto-shrinking decimals to fit the
//! column) is not modelled — at extraction time the column width is not
//! available, and SheetJS / Apache POI both default to the
//! width-independent rules above for headless rendering.
//!
//! # Why this is not just `format!("{}")`
//!
//! Rust's default float formatter emits the shortest round-trippable
//! representation, which differs from Excel in a half-dozen places:
//! it never elides trailing zeros from a fractional part, it picks
//! scientific only at far-extreme magnitudes, and it doesn't constrain
//! to 11 significant digits. Letting it through would silently change
//! the displayed text of every General-formatted cell relative to what
//! the user authored in Excel.
//!
//! # Relationship to the degradation contract
//!
//! Pure formatter — no warnings raised. The caller (today the
//! [`super::format_value`] scaffold; eventually the emit pass in #46)
//! decides whether to flag the General path with a telemetry warning;
//! this module just hands back a string.

use std::fmt::Write as _;

/// Render a value against Excel's "General" format.
///
/// See the module docstring for the full rule table. Always returns a
/// non-empty string for finite inputs; non-finite inputs (NaN / ±∞)
/// fall back to `"#NUM!"`, matching Excel's display for those cells.
pub(super) fn format_general(value: f64) -> String {
    if !value.is_finite() {
        // Excel surfaces evaluation errors with `#NUM!` / `#DIV/0!`
        // sigils. We don't carry the original error kind through the
        // serial, so use the most common one.
        return "#NUM!".to_string();
    }

    if value == 0.0 {
        return "0".to_string();
    }

    let abs_val = value.abs();
    let exp = abs_val.log10().floor() as i32;

    // Magnitude check comes before the integer shortcut: a value like
    // 2.918e13 is exactly-representable as an `i64` and has `fract() == 0`,
    // but Excel still switches to scientific because the integer form
    // would require 14 digits (above the 11-sig-digit cap). Put another
    // way, the integer shortcut only applies in the "fits in General's
    // digit budget" band, which is exactly the fixed-point band
    // `-4 <= exp < 11`.
    if !(-4..11).contains(&exp) {
        return format_scientific(value);
    }

    // Integer shortcut — no decimal point needed when fract() == 0. The
    // magnitude guard above ensures |v| < 1e11 here, well within i64 range.
    if value.fract() == 0.0 {
        return format!("{}", value as i64);
    }

    // Fixed-point. Excel's General caps at 11 significant digits total;
    // for an integer part of `exp + 1` digits, that leaves
    // `10 - exp` fractional digits.
    let digits_after = (10 - exp).max(0) as usize;
    let raw = format!("{:.*}", digits_after, value);
    trim_trailing_fractional_zeros(&raw)
}

/// Excel's General scientific form: `<mantissa>E±NN` with up to 6
/// significant digits in the mantissa. Mantissa is rounded
/// half-away-from-zero, with carry overflow propagated to the
/// exponent (e.g. 9.99999e5 → 1E+06).
fn format_scientific(value: f64) -> String {
    let neg = value < 0.0;
    let abs_val = value.abs();
    let raw_exp = abs_val.log10().floor() as i32;
    let mantissa_raw = abs_val / 10f64.powi(raw_exp);

    // Round to 5 decimal places (6 significant digits total). Use
    // half-away-from-zero — Rust's `f64::round` matches this.
    let mantissa_rounded = (mantissa_raw * 1e5).round() / 1e5;

    let (mantissa, exp) = if mantissa_rounded >= 10.0 {
        (mantissa_rounded / 10.0, raw_exp + 1)
    } else {
        (mantissa_rounded, raw_exp)
    };

    let mantissa_str = trim_trailing_fractional_zeros(&format!("{:.5}", mantissa));

    let mut out = String::new();
    if neg {
        out.push('-');
    }
    out.push_str(&mantissa_str);
    let _ = write!(out, "E{}{:02}", if exp >= 0 { '+' } else { '-' }, exp.abs());
    out
}

/// Drop trailing zeros from the fractional part, then drop the
/// decimal point itself if nothing follows it. Leaves leading zeros
/// (`"0.5"` → `"0.5"`, not `".5"`) and integer parts untouched
/// (`"1234"` → `"1234"`).
fn trim_trailing_fractional_zeros(s: &str) -> String {
    let Some(dot_idx) = s.find('.') else {
        return s.to_string();
    };
    let trimmed = s.trim_end_matches('0');
    // If trimming consumed every fractional digit, also drop the
    // decimal point so `1.000` → `1` rather than `1.`.
    if trimmed.len() == dot_idx + 1 {
        return s[..dot_idx].to_string();
    }
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Zero & integer shortcuts ------------------------------------------

    #[test]
    fn zero_renders_as_bare_zero() {
        assert_eq!(format_general(0.0), "0");
    }

    #[test]
    fn negative_zero_renders_as_bare_zero() {
        // -0.0 is bit-distinct from 0.0 but Excel collapses both to "0".
        assert_eq!(format_general(-0.0), "0");
    }

    #[test]
    fn small_positive_integer() {
        assert_eq!(format_general(1.0), "1");
        assert_eq!(format_general(42.0), "42");
        assert_eq!(format_general(1234.0), "1234");
    }

    #[test]
    fn small_negative_integer() {
        assert_eq!(format_general(-7.0), "-7");
        assert_eq!(format_general(-1234.0), "-1234");
    }

    #[test]
    fn ten_digit_integer_renders_as_integer() {
        assert_eq!(format_general(1_234_567_890.0), "1234567890");
    }

    // --- Fixed-point with trim ---------------------------------------------

    #[test]
    fn one_decimal_place() {
        assert_eq!(format_general(1.5), "1.5");
    }

    #[test]
    fn trailing_zeros_are_trimmed() {
        // 1.5 stored as exact half — fixed-point with 9 decimals is
        // "1.500000000"; trim back to "1.5".
        assert_eq!(format_general(1.5), "1.5");
        assert_eq!(format_general(2.25), "2.25");
    }

    #[test]
    fn small_decimal_above_threshold_uses_fixed() {
        // 0.001 is at exp = -3, above the -4 scientific threshold.
        assert_eq!(format_general(0.001), "0.001");
        assert_eq!(format_general(0.0001), "0.0001");
    }

    #[test]
    fn negative_fixed_point() {
        assert_eq!(format_general(-1.5), "-1.5");
        assert_eq!(format_general(-0.25), "-0.25");
    }

    #[test]
    fn fractional_close_to_one_keeps_significant_digits() {
        // 0.9999999999 has 10 significant digits; should render as
        // "0.9999999999".
        assert_eq!(format_general(0.9999999999), "0.9999999999");
    }

    // --- Scientific notation -----------------------------------------------

    #[test]
    fn very_large_uses_scientific() {
        // 1e11 is the boundary — exp == 11 triggers scientific.
        assert_eq!(format_general(1e11), "1E+11");
    }

    #[test]
    fn very_large_with_mantissa() {
        assert_eq!(format_general(2.918e13), "2.918E+13");
    }

    #[test]
    fn very_small_uses_scientific() {
        // 1e-5 is below the -4 threshold.
        assert_eq!(format_general(1e-5), "1E-05");
        assert_eq!(format_general(5.2e-5), "5.2E-05");
    }

    #[test]
    fn scientific_carry_propagates_to_exponent() {
        // 9.999999e11 sits above the scientific threshold; the mantissa
        // 9.999999 rounds to 10.00000 at 5 decimals, which carries into
        // the exponent: the final rendering is 1E+12 rather than
        // 10E+11, matching POI and Excel.
        assert_eq!(format_general(9.999999e11), "1E+12");
    }

    #[test]
    fn scientific_negative_mantissa() {
        assert_eq!(format_general(-2.918e13), "-2.918E+13");
        assert_eq!(format_general(-5.2e-5), "-5.2E-05");
    }

    #[test]
    fn scientific_six_significant_digits_max() {
        // Mantissa exceeding 6 sig digits gets rounded.
        // 1.2345678e12 → mantissa 1.23457, exp 12.
        assert_eq!(format_general(1.2345678e12), "1.23457E+12");
    }

    #[test]
    fn integer_at_or_above_1e11_uses_scientific() {
        // Boundary: 12-digit integer no longer fits as integer; should
        // route through scientific.
        assert_eq!(format_general(123_456_789_012.0), "1.23457E+11");
    }

    // --- Non-finite inputs -------------------------------------------------

    #[test]
    fn nan_renders_as_num_error() {
        assert_eq!(format_general(f64::NAN), "#NUM!");
    }

    #[test]
    fn positive_infinity_renders_as_num_error() {
        assert_eq!(format_general(f64::INFINITY), "#NUM!");
    }

    #[test]
    fn negative_infinity_renders_as_num_error() {
        assert_eq!(format_general(f64::NEG_INFINITY), "#NUM!");
    }
}
