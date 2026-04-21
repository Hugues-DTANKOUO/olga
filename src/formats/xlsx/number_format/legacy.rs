//! Excel cell number-format classifier.
//!
//! Excel's data model separates the raw numeric value of a cell from its
//! *display format*. A cell containing `0.0261111` styled with `0.0%` is
//! shown to the user as `2.6%`; the raw value and the displayed value are
//! *both* correct answers to "what is in this cell?" — they just answer
//! different questions. For a text extractor meant to feed downstream
//! AI / report-reading use cases, the user-facing displayed value is
//! almost always what's wanted: a budget readout of `-2.6%` means
//! something; `-0.0261111111111111` does not.
//!
//! Rendering itself is handled by the POI-aligned engine rooted at
//! [`super::format_value`] — this module survives because two decisions
//! upstream still depend on the coarse category rather than on the raw
//! format code:
//!
//! * `sheets::raw::read_raw_cell_content` short-circuits
//!   [`CellNumberFormat::Default`] and [`CellNumberFormat::Text`] back to
//!   the raw `value_text` so unstyled and `@`-styled numerics emit their
//!   stored representation verbatim (the POI engine would otherwise
//!   re-render `5` as `"5"` via General, identical in practice but more
//!   work per cell).
//! * `sheets::raw::cell_requires_workbook_decoder` uses
//!   [`CellNumberFormat::Date`] as the routing signal for cell-type
//!   dispatch.
//!
//! Both consumers only need the category; neither consumes the raw
//! format code. The classifier stays minimal on purpose.

/// Category of an Excel cell number format.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) enum CellNumberFormat {
    /// No applicable formatting — render raw.
    #[default]
    Default,
    /// `@` text format — force string interpretation.
    Text,
    /// Any date / time / datetime style — render via ISO conversion.
    Date,
    /// `0%`, `0.0%`, `0.00%` — render the value * 100 with a % suffix.
    Percent { decimals: u8 },
    /// Currency: fixed prefix/suffix text (e.g. `$`, ` €`), decimals,
    /// thousands separator, and whether negatives are wrapped in parens.
    Currency {
        prefix: String,
        suffix: String,
        decimals: u8,
        thousands_sep: bool,
        negative_parens: bool,
    },
    /// Plain number with explicit decimals / thousands grouping.
    Number { decimals: u8, thousands_sep: bool },
}

/// Classify an Excel number-format code into a [`CellNumberFormat`].
///
/// The parser handles:
///
/// * The built-in currency / percent / number format patterns used by
///   modern locales (`$#,##0`, `#,##0.00_);[Red](#,##0.00)`, `0.0%`,
///   `#,##0`).
/// * Locale-code currency prefixes like `[$$-409]` (US dollar),
///   `[$€-40C]` (euro), `[$£-809]` (pound). The locale id is dropped;
///   the currency symbol is preserved.
/// * Quoted literal runs (`"USD "#,##0.00`).
/// * Negative-section detection (`positive;negative;…`) including the
///   common `[Red]` colour code.
/// * `_` pad directives and `*` repeat directives (consumed, not
///   rendered).
///
/// Unrecognised, overly-complex, or conditional (`[>1000]`) formats
/// classify as [`CellNumberFormat::Default`] — the same fallback as the
/// pre-format-preservation behaviour, so they are strictly non-regressive.
pub(crate) fn parse_excel_number_format(format_code: &str) -> CellNumberFormat {
    let code = format_code.trim();
    if code.is_empty() || code.eq_ignore_ascii_case("general") {
        return CellNumberFormat::Default;
    }
    if code == "@" {
        return CellNumberFormat::Text;
    }

    let normalized = normalize_format_code(code);
    if normalized.contains("am/pm")
        || has_datetime_token(&normalized)
        // A format like `[h]:mm` is elapsed hours — still a duration
        // render, which the date path handles.
        || normalized.contains("[h]")
        || normalized.contains("[m]")
        || normalized.contains("[s]")
    {
        return CellNumberFormat::Date;
    }

    // Grab only the positive section — Excel applies the second (`;`-separated)
    // section to negatives, but the positive section dictates category.
    let first_section = split_sections(code).next().unwrap_or(code);
    let has_negative_parens = has_negative_paren_section(code);
    let (prefix, suffix) = extract_currency_affixes(first_section);
    let is_currency = !prefix.is_empty() || !suffix.is_empty();
    let core = strip_affixes(first_section, &prefix, &suffix);

    if core.contains('%') {
        let decimals = count_fraction_digits(&core);
        return CellNumberFormat::Percent { decimals };
    }

    let decimals = count_fraction_digits(&core);
    let thousands_sep = core.contains(',');

    if is_currency {
        return CellNumberFormat::Currency {
            prefix,
            suffix,
            decimals,
            thousands_sep,
            negative_parens: has_negative_parens,
        };
    }

    // Plain number — only treat it as such if the format actually has
    // digit placeholders. Otherwise default.
    if core.chars().any(|c| matches!(c, '0' | '#')) {
        return CellNumberFormat::Number {
            decimals,
            thousands_sep,
        };
    }

    CellNumberFormat::Default
}

// ---------------------------------------------------------------------------
// Parser helpers
// ---------------------------------------------------------------------------

/// Lowercase the format code, eliding quoted literals, `[…]` brackets
/// except the `[$…]`/`[h]`/`[m]`/`[s]`/colour forms that are used for
/// date/time duration detection and currency. The result is a canonical
/// form suitable for coarse substring matching (datetime, percent).
fn normalize_format_code(code: &str) -> String {
    let mut out = String::with_capacity(code.len());
    let mut chars = code.chars().peekable();
    let mut in_quotes = false;
    let mut bracket_depth = 0u32;
    while let Some(c) = chars.next() {
        match c {
            '"' => in_quotes = !in_quotes,
            '\\' => {
                let _ = chars.next();
            }
            '[' if !in_quotes => {
                bracket_depth += 1;
                // Preserve bracketed content verbatim (as lowercase) so
                // elapsed-time markers like `[h]` survive for datetime
                // detection.
                out.push('[');
            }
            ']' if !in_quotes && bracket_depth > 0 => {
                bracket_depth -= 1;
                out.push(']');
            }
            '_' | '*' if !in_quotes => {
                // `_c` means "pad width of c"; `*c` means "repeat c"; both
                // consume the next char and emit nothing visible.
                let _ = chars.next();
            }
            _ if in_quotes => {}
            _ => out.push(c.to_ascii_lowercase()),
        }
    }
    out
}

fn has_datetime_token(normalized: &str) -> bool {
    // `y`, `d` are unambiguous. `m` can mean month OR minute; `h`+`m`
    // together (`hh:mm`) makes it time. `s` alone is second. Be slightly
    // generous here — we just classify, we don't render.
    let has_y = normalized.contains('y');
    let has_d = normalized.contains('d');
    let has_h = normalized.contains('h');
    let has_s = normalized.contains('s');
    let has_m = normalized.contains('m') && (has_y || has_d || has_h || has_s);
    has_y || has_d || has_h || has_s || has_m
}

/// Split a format string into its sections on `;` — Excel allows up to
/// four (positive;negative;zero;text). Skip `;` inside brackets and
/// quoted literals.
fn split_sections(code: &str) -> impl Iterator<Item = &str> {
    let mut sections: Vec<&str> = Vec::with_capacity(4);
    let bytes = code.as_bytes();
    let mut start = 0;
    let mut in_quotes = false;
    let mut bracket_depth = 0u32;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        match c {
            b'"' => in_quotes = !in_quotes,
            b'\\' => {
                i += 1;
            }
            b'[' if !in_quotes => bracket_depth += 1,
            b']' if !in_quotes && bracket_depth > 0 => bracket_depth -= 1,
            b';' if !in_quotes && bracket_depth == 0 => {
                sections.push(&code[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    sections.push(&code[start..]);
    sections.into_iter()
}

/// Scan all sections and check whether any non-positive section wraps the
/// number body in parentheses — the conventional accounting-style format
/// for negative amounts. Matches `($#,##0)` and `(#,##0)`.
fn has_negative_paren_section(code: &str) -> bool {
    for (idx, section) in split_sections(code).enumerate() {
        if idx == 0 {
            continue;
        }
        let cleaned = strip_brackets(section);
        let trimmed = cleaned.trim();
        if trimmed.starts_with('(') && trimmed.ends_with(')') {
            return true;
        }
    }
    false
}

fn strip_brackets(section: &str) -> String {
    let mut out = String::with_capacity(section.len());
    let mut depth = 0u32;
    let mut in_quotes = false;
    let mut chars = section.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => in_quotes = !in_quotes,
            '\\' => {
                let _ = chars.next();
            }
            '[' if !in_quotes => depth += 1,
            ']' if !in_quotes && depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// Extract currency prefix (before digit placeholders) and suffix (after).
/// Handles `"$"#,##0`, `$#,##0`, `#,##0" €"`, and `[$$-409]#,##0`.
fn extract_currency_affixes(section: &str) -> (String, String) {
    // Find the first digit placeholder after optional affix markers.
    let chars: Vec<char> = section.chars().collect();
    let len = chars.len();

    let mut i = 0;
    let mut prefix = String::new();
    while i < len {
        let c = chars[i];
        match c {
            '"' => {
                // Quoted literal: consume until matching "
                i += 1;
                while i < len && chars[i] != '"' {
                    prefix.push(chars[i]);
                    i += 1;
                }
                if i < len {
                    i += 1;
                }
            }
            '[' => {
                // `[$SYMBOL-LOCALE]` → extract SYMBOL.
                let close = chars[i + 1..].iter().position(|&c| c == ']');
                if let Some(offset) = close {
                    let inner: String = chars[i + 1..i + 1 + offset].iter().collect();
                    if let Some(rest) = inner.strip_prefix('$') {
                        let sym = match rest.find('-') {
                            Some(dash) => &rest[..dash],
                            None => rest,
                        };
                        prefix.push_str(sym);
                    }
                    // `[Red]`, `[Blue]`, `[h]`, etc. — ignore for currency.
                    i += offset + 2;
                } else {
                    i += 1;
                }
            }
            '\\' => {
                if i + 1 < len {
                    prefix.push(chars[i + 1]);
                    i += 2;
                } else {
                    i += 1;
                }
            }
            '$' | '€' | '£' | '¥' | '¢' | '₹' | '₽' | '₩' => {
                prefix.push(c);
                i += 1;
            }
            ' ' | '(' => {
                prefix.push(c);
                i += 1;
            }
            '0' | '#' | '?' => break,
            _ => {
                // Any other char before digits is ambient text — probably
                // not currency, bail.
                return (String::new(), String::new());
            }
        }
    }

    // Now scan forward past the numeric body. The body stops at the
    // first non-placeholder character — whatever follows is the suffix
    // (which may be a quoted literal, a currency glyph, or a bracket).
    let mut j = i;
    while j < len {
        match chars[j] {
            '0' | '#' | '?' | ',' | '.' | '%' => j += 1,
            _ => break,
        }
    }

    let mut suffix = String::new();
    while j < len {
        let c = chars[j];
        match c {
            '"' => {
                j += 1;
                while j < len && chars[j] != '"' {
                    suffix.push(chars[j]);
                    j += 1;
                }
                if j < len {
                    j += 1;
                }
            }
            '\\' => {
                if j + 1 < len {
                    suffix.push(chars[j + 1]);
                    j += 2;
                } else {
                    j += 1;
                }
            }
            '$' | '€' | '£' | '¥' | '¢' | '₹' | '₽' | '₩' => {
                suffix.push(c);
                j += 1;
            }
            ' ' | ')' => {
                suffix.push(c);
                j += 1;
            }
            _ => j += 1,
        }
    }

    let prefix = prefix.trim().to_string();
    let suffix = suffix.trim().to_string();
    // A format with `%` is NOT currency even if it has a `$` prefix (which
    // realistically never happens, but stay strict).
    if section.contains('%') {
        return (String::new(), String::new());
    }
    (prefix, suffix)
}

fn strip_affixes(section: &str, prefix: &str, suffix: &str) -> String {
    let cleaned = strip_brackets(section);
    let mut s = cleaned.trim().to_string();
    if !prefix.is_empty()
        && let Some(rest) = s.strip_prefix(prefix)
    {
        s = rest.trim().to_string();
    }
    if !suffix.is_empty()
        && let Some(rest) = s.strip_suffix(suffix)
    {
        s = rest.trim().to_string();
    }
    s
}

fn count_fraction_digits(core: &str) -> u8 {
    // Look for the first `.`; count trailing `0` / `#` / `?` up to the
    // next separator (`;`, `%`, end of string).
    let idx = match core.find('.') {
        Some(i) => i,
        None => return 0,
    };
    let mut n: u8 = 0;
    for c in core[idx + 1..].chars() {
        match c {
            '0' | '#' | '?' => n = n.saturating_add(1),
            _ => break,
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_formats() {
        assert_eq!(parse_excel_number_format(""), CellNumberFormat::Default);
        assert_eq!(
            parse_excel_number_format("General"),
            CellNumberFormat::Default
        );
        assert_eq!(parse_excel_number_format("@"), CellNumberFormat::Text);
    }

    #[test]
    fn date_formats_classify() {
        for code in [
            "yyyy-mm-dd",
            "m/d/yyyy",
            "dddd, mmmm d, yyyy",
            "h:mm:ss",
            "hh:mm AM/PM",
            "[h]:mm",
            "d-mmm-yy",
        ] {
            assert_eq!(
                parse_excel_number_format(code),
                CellNumberFormat::Date,
                "expected Date for {code}"
            );
        }
    }

    #[test]
    fn percent_formats_classify() {
        assert_eq!(
            parse_excel_number_format("0%"),
            CellNumberFormat::Percent { decimals: 0 }
        );
        assert_eq!(
            parse_excel_number_format("0.0%"),
            CellNumberFormat::Percent { decimals: 1 }
        );
        assert_eq!(
            parse_excel_number_format("0.00%"),
            CellNumberFormat::Percent { decimals: 2 }
        );
        // Budget.xlsx variance column
        assert_eq!(
            parse_excel_number_format("0.0%;(0.0%)"),
            CellNumberFormat::Percent { decimals: 1 }
        );
    }

    #[test]
    fn currency_formats_classify() {
        assert_eq!(
            parse_excel_number_format("$#,##0"),
            CellNumberFormat::Currency {
                prefix: "$".to_string(),
                suffix: String::new(),
                decimals: 0,
                thousands_sep: true,
                negative_parens: false,
            }
        );
        assert_eq!(
            parse_excel_number_format("$#,##0.00"),
            CellNumberFormat::Currency {
                prefix: "$".to_string(),
                suffix: String::new(),
                decimals: 2,
                thousands_sep: true,
                negative_parens: false,
            }
        );
        assert_eq!(
            parse_excel_number_format("$#,##0.00;($#,##0.00)"),
            CellNumberFormat::Currency {
                prefix: "$".to_string(),
                suffix: String::new(),
                decimals: 2,
                thousands_sep: true,
                negative_parens: true,
            }
        );
        // Locale-coded currency prefix
        let locale_code = parse_excel_number_format("[$$-409]#,##0.00");
        assert_eq!(
            locale_code,
            CellNumberFormat::Currency {
                prefix: "$".to_string(),
                suffix: String::new(),
                decimals: 2,
                thousands_sep: true,
                negative_parens: false,
            }
        );
        // Suffixed euro: `#,##0 €`
        let euro_suffix = parse_excel_number_format("#,##0\" €\"");
        assert_eq!(
            euro_suffix,
            CellNumberFormat::Currency {
                prefix: String::new(),
                suffix: "€".to_string(),
                decimals: 0,
                thousands_sep: true,
                negative_parens: false,
            }
        );
    }

    #[test]
    fn plain_number_formats_classify() {
        assert_eq!(
            parse_excel_number_format("0"),
            CellNumberFormat::Number {
                decimals: 0,
                thousands_sep: false,
            }
        );
        assert_eq!(
            parse_excel_number_format("0.00"),
            CellNumberFormat::Number {
                decimals: 2,
                thousands_sep: false,
            }
        );
        assert_eq!(
            parse_excel_number_format("#,##0.000"),
            CellNumberFormat::Number {
                decimals: 3,
                thousands_sep: true,
            }
        );
    }
}
