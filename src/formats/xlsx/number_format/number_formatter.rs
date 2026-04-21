//! POI-aligned CellNumberFormatter — renders a Number-kind section body.
//!
//! # Responsibility
//!
//! A single entry point — [`format_number_body`] — consumes:
//!
//! * `abs_value` — the non-negative numeric value to render (the caller is
//!   expected to strip the sign; POI's section layer decides whether to
//!   pre-negate based on each section's `has_sign()` flag).
//! * `prepend_minus` — `true` iff the original value was negative *and*
//!   the chosen section's body does not carry its own literal sign.
//!   That's the POI-aligned way of preventing `-+1bps`-style double
//!   signing: when the negative section contains a literal `-`, the body
//!   renders the minus itself and we pass `prepend_minus = false`.
//! * `body` — the body text of the chosen `FormatSection`, already
//!   stripped of bracket prefixes by the section splitter (#39).
//!
//! # Grammar supported in this pass (#40 + #48)
//!
//! * Digit placeholders: `0` (required, pad with zero), `#` (optional,
//!   suppress when absent), `?` (space-padded for column alignment).
//! * Decimal separator `.`.
//! * Grouping comma `,` between digit placeholders — emits thousand
//!   separators in the output (including for overflow digits).
//! * Scaling commas — trailing `,` after the last digit placeholder in
//!   the integer part; each comma divides the value by 1000 before
//!   rendering. A format like `#,##0,,` scales by one million.
//! * Percent `%` — multiplies the value by 100 per `%` seen (Excel
//!   allows multiple `%` in a format code; each compounds).
//! * Exponent `E+` (always show sign) and `E-` (show only negative).
//!   The mantissa digit count preceding `E` drives normalisation, and
//!   the digit count after `E` drives zero-padding of the exponent.
//! * Fractions (`# ?/?`, `# ?/8`, `?/16`, `# ??/??`, …). When the body
//!   contains an unquoted `/`, the tokenizer produces a [`Token::Slash`]
//!   flanked by the numerator placeholder run and either a
//!   [`Token::FixedDenom`] (integer literal denominator like `?/8`) or a
//!   placeholder run (`?/?` → auto-denominator best-approximation bounded
//!   by the placeholder count). The renderer splits `abs_value` into
//!   whole + fractional part, picks the best rational approximation, and
//!   emits `whole num/denom` with the format's separator literals. Both
//!   Fixed and Auto modes emit the fraction in reduced form per the
//!   OpenXML convention shared by Excel and LibreOffice — `0.25` with
//!   `?/8` renders `1/4`, not `2/8` — which for Fixed mode means the
//!   rendered denominator is the literal denominator divided by
//!   `gcd(num, denom)`. This is what lets `96.125` with `# ?/8` render
//!   as `96 1/8` (already reduced) and `96.250` render as `96 1/4`
//!   instead of the legacy `96.125` / `96.250` passthrough.
//! * Literal runs:
//!   * quoted `"..."` — POI-aligned lenient handling: an unterminated
//!     `"` at end-of-body is treated as an opening quote whose
//!     remainder is a plain literal (so `# ?/8"` renders the inch-mark
//!     suffix verbatim instead of failing the whole cell),
//!   * backslash-escaped `\x`,
//!   * intrinsic literal chars (`$`, `-`, `+`, `(`, `)`, `:`, space, and
//!     the handful of other ASCII punctuation Excel renders verbatim).
//! * Padding `_x` — the next character is a column-width spacer in Excel;
//!   we emit a single space because column widths aren't available at
//!   extraction time.
//! * Fill `*x` — the next character is a fill directive; we drop it
//!   (same reason).
//!
//! # Out of scope
//!
//! * Literals wedged **between** digit placeholders (e.g. `0"mid"0`) —
//!   we render them once at the first-digit position rather than
//!   mid-stream. This is pathological enough that POI itself has
//!   quirks; we'll tighten fidelity when the corpus shows a
//!   regression.
//!
//! # Relationship to the degradation contract
//!
//! This module's sole caller is the future `format_value` dispatch (wired
//! in #46). Errors surface as [`FormatWarning`] so the contract's telemetry
//! channel counts them by [`FormatWarningKind`] alongside all other
//! per-feature warnings the engine can raise.

use super::contract::{FormatWarning, FormatWarningKind};

/// Internal token type for a tokenized Number body.
///
/// Kept private: every consumer goes through [`format_number_body`]. The
/// enum is small enough that exposing it would be more liability than
/// help — the POI grammar is expressive enough that callers wanting to
/// probe the parse should read tests instead.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Digit(DigitPlace),
    Decimal,
    Group,
    Percent,
    Exponent(ExpSign),
    Literal(String),
    /// POI's `_x` column-aligner; we emit a single space.
    Pad,
    /// POI's `*x` fill directive; we drop (no column width to fill).
    Fill,
    /// Fraction marker `/`. Present only when the body carries a
    /// fraction grammar like `# ?/?` or `?/8`; the renderer routes to
    /// [`render_fraction`] whenever this token is in the stream.
    Slash,
    /// Fixed denominator parsed immediately after a `/`. `?/8` yields
    /// [`Token::Slash`] followed by `Token::FixedDenom(8)`. Kept as its
    /// own variant (rather than a plain `Literal("8")`) so the suffix
    /// literal after the denominator (e.g. the `"` inch mark in
    /// `# ?/8\"`) stays structurally separate and `push_literal_char`
    /// doesn't merge the two.
    FixedDenom(u32),
}

/// Digit-placeholder kind, one-to-one with Excel's `0`/`#`/`?`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DigitPlace {
    /// `0` — always emit a digit; pad with `0` when the value is short.
    Required,
    /// `#` — emit a digit when present; suppress (no char) when the
    /// value is shorter than the placeholder run. Leading-zero
    /// suppression for a whole-number zero also lives here.
    Optional,
    /// `?` — emit a digit when present; pad with a space when not
    /// (column-alignment behaviour).
    Space,
}

/// Exponent-sign mode, from the character following `E` in the format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpSign {
    /// `E+` — emit `+` or `-` explicitly for every exponent.
    Always,
    /// `E-` — emit `-` only for negative exponents; no sign otherwise.
    MinusOnly,
}

/// Aggregate parse-time descriptor; drives rendering without re-scanning
/// the token stream on the hot path.
struct Desc {
    /// Count of digit placeholders in the integer part.
    int_placeholders: usize,
    /// Count of digit placeholders after `.` and before `E`.
    frac_placeholders: usize,
    /// Count of digit placeholders after `E`.
    exp_placeholders: usize,
    /// `true` iff any grouping comma sits between two digit placeholders
    /// in the integer part (POI's "group separator in effect" rule).
    use_grouping: bool,
    /// Trailing commas in the integer part (after the last digit
    /// placeholder) that scale the value down by 1000 each.
    scaling_commas: usize,
    /// Number of `%` markers; each multiplies the value by 100.
    percent_count: usize,
    /// `true` iff an `E` marker is present; drives scientific rendering.
    has_exponent: bool,
    /// Sign style for the exponent when `has_exponent` is set.
    exp_sign_style: ExpSign,
    /// Cached: placeholder layout of the integer part in order (used
    /// during emission to decide pad chars and leading-zero suppression).
    int_placeholder_list: Vec<DigitPlace>,
    /// Cached: placeholder layout of the fractional part in order (used
    /// for trailing-zero suppression of `#` placeholders).
    frac_placeholder_list: Vec<DigitPlace>,
}

/// Render a Number-kind section body against a non-negative value.
///
/// See the module docstring for the grammar supported. Returns `Err` only
/// when the body is syntactically broken (unterminated quote, trailing
/// backslash) or when it carries a `/` marker that doesn't resolve to a
/// well-formed numerator/denominator pair (e.g. `/8` with no numerator
/// placeholder, or `?/0`).
pub(super) fn format_number_body(
    abs_value: f64,
    prepend_minus: bool,
    body: &str,
) -> Result<String, FormatWarning> {
    let tokens = tokenize(body)?;
    if tokens.iter().any(|t| matches!(t, Token::Slash)) {
        return match detect_fraction(&tokens) {
            Some(layout) => Ok(render_fraction(abs_value, prepend_minus, &tokens, &layout)),
            None => Err(FormatWarning::new(
                FormatWarningKind::FormatFeatureFraction,
                body,
                format!("{abs_value}"),
                "fraction body lacks a numerator placeholder or a valid denominator",
            )),
        };
    }
    let desc = describe(&tokens);
    Ok(render(abs_value, prepend_minus, &tokens, &desc))
}

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

fn tokenize(body: &str) -> Result<Vec<Token>, FormatWarning> {
    let mut tokens: Vec<Token> = Vec::new();
    let mut chars = body.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '0' => tokens.push(Token::Digit(DigitPlace::Required)),
            '#' => tokens.push(Token::Digit(DigitPlace::Optional)),
            '?' => tokens.push(Token::Digit(DigitPlace::Space)),
            '.' => tokens.push(Token::Decimal),
            ',' => tokens.push(Token::Group),
            '%' => tokens.push(Token::Percent),
            'E' | 'e' => {
                let sign = match chars.peek() {
                    Some('+') => {
                        chars.next();
                        ExpSign::Always
                    }
                    Some('-') => {
                        chars.next();
                        ExpSign::MinusOnly
                    }
                    _ => {
                        // No sign marker follows — this isn't a POI-style
                        // exponent. Treat the `E` as a literal so quirky
                        // formats (like `E5` embedded in a prefix) render
                        // as text instead of blowing up the stream.
                        push_literal_char(&mut tokens, c);
                        continue;
                    }
                };
                tokens.push(Token::Exponent(sign));
            }
            '"' => {
                let mut lit = String::new();
                let mut closed = false;
                for next in chars.by_ref() {
                    if next == '"' {
                        closed = true;
                        break;
                    }
                    lit.push(next);
                }
                if !closed {
                    // POI-aligned lenient tokenization: an unterminated
                    // `"` at end-of-body is common in hand-typed engineering
                    // formats (e.g. `# ?/8"` for an inch-mark suffix). POI's
                    // regex-based tokenizer `"[^"]*"` simply skips the
                    // unmatched segment; we mirror that by treating the
                    // opening `"` and whatever follows as a literal run.
                    // The alternative — rejecting the whole code — would
                    // silently drop the format for a whole column of cells,
                    // which the degradation contract explicitly rules out.
                    push_literal_char(&mut tokens, '"');
                    push_literal_str(&mut tokens, &lit);
                } else {
                    push_literal_str(&mut tokens, &lit);
                }
            }
            '\\' => {
                let Some(next) = chars.next() else {
                    return Err(FormatWarning::new(
                        FormatWarningKind::MalformedFormatCode,
                        body,
                        String::new(),
                        "trailing backslash in number body",
                    ));
                };
                push_literal_char(&mut tokens, next);
            }
            '_' => {
                // Consume the width-reference char (POI's `_x` padding).
                let _ = chars.next();
                tokens.push(Token::Pad);
            }
            '*' => {
                // Consume the fill char (POI's `*x` fill).
                let _ = chars.next();
                tokens.push(Token::Fill);
            }
            '/' => {
                // Fraction marker. Push Slash, then — if the next run is
                // an integer literal — consume it as a FixedDenom so the
                // common `?/8`, `?/16`, `?/100`, `?/1000` forms parse
                // cleanly. If placeholders (`?`, `#`, `0`) follow
                // instead, they fall through to the main loop and form
                // an Auto-denominator run (`?/?`, `?/??`).
                tokens.push(Token::Slash);
                let mut denom_text = String::new();
                while let Some(&nc) = chars.peek() {
                    if nc.is_ascii_digit() {
                        denom_text.push(nc);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if !denom_text.is_empty()
                    && let Ok(n) = denom_text.parse::<u32>()
                    && n > 0
                {
                    tokens.push(Token::FixedDenom(n));
                }
                // If the denom parse fails or is zero, no FixedDenom is
                // pushed; `detect_fraction` will return `None` and the
                // caller surfaces a `FormatFeatureFraction` warning.
            }
            '$' | '-' | '+' | '(' | ')' | ':' | ' ' | '\'' | '!' | '^' | '&' | '~' | '{' | '}'
            | '<' | '>' | '=' => {
                push_literal_char(&mut tokens, c);
            }
            _ => {
                // Unknown char: render verbatim. Better than erroring on
                // future grammar features we haven't seen yet.
                push_literal_char(&mut tokens, c);
            }
        }
    }
    Ok(tokens)
}

fn push_literal_char(tokens: &mut Vec<Token>, c: char) {
    if let Some(Token::Literal(last)) = tokens.last_mut() {
        last.push(c);
    } else {
        tokens.push(Token::Literal(c.to_string()));
    }
}

fn push_literal_str(tokens: &mut Vec<Token>, s: &str) {
    if s.is_empty() {
        return;
    }
    if let Some(Token::Literal(last)) = tokens.last_mut() {
        last.push_str(s);
    } else {
        tokens.push(Token::Literal(s.to_string()));
    }
}

// ---------------------------------------------------------------------------
// Descriptor
// ---------------------------------------------------------------------------

fn describe(tokens: &[Token]) -> Desc {
    let dec_idx = tokens.iter().position(|t| matches!(t, Token::Decimal));
    let exp_idx = tokens.iter().position(|t| matches!(t, Token::Exponent(_)));
    let int_end = dec_idx.or(exp_idx).unwrap_or(tokens.len());
    let int_tokens = &tokens[..int_end];

    let last_digit_in_int = int_tokens
        .iter()
        .rposition(|t| matches!(t, Token::Digit(_)));

    let int_placeholder_list: Vec<DigitPlace> = int_tokens
        .iter()
        .filter_map(|t| match t {
            Token::Digit(p) => Some(*p),
            _ => None,
        })
        .collect();
    let int_placeholders = int_placeholder_list.len();

    // Grouping: any `,` with a digit placeholder to its right in the int
    // section. Otherwise it's either a scaling comma (trailing) or a
    // decorative literal (rare).
    let use_grouping = int_tokens
        .iter()
        .enumerate()
        .any(|(i, t)| matches!(t, Token::Group) && last_digit_in_int.is_some_and(|ld| i < ld));

    // Scaling: trailing commas after the last digit placeholder.
    let scaling_commas = match last_digit_in_int {
        Some(ld) => int_tokens
            .iter()
            .skip(ld + 1)
            .filter(|t| matches!(t, Token::Group))
            .count(),
        None => 0,
    };

    let (frac_placeholder_list, frac_placeholders): (Vec<DigitPlace>, usize) =
        match (dec_idx, exp_idx) {
            (Some(d), Some(e)) if e > d => {
                let list: Vec<DigitPlace> = tokens[d + 1..e]
                    .iter()
                    .filter_map(|t| match t {
                        Token::Digit(p) => Some(*p),
                        _ => None,
                    })
                    .collect();
                let n = list.len();
                (list, n)
            }
            (Some(d), None) => {
                let list: Vec<DigitPlace> = tokens[d + 1..]
                    .iter()
                    .filter_map(|t| match t {
                        Token::Digit(p) => Some(*p),
                        _ => None,
                    })
                    .collect();
                let n = list.len();
                (list, n)
            }
            _ => (Vec::new(), 0),
        };

    let (has_exponent, exp_sign_style, exp_placeholders) = match exp_idx {
        Some(e) => {
            let sign = match &tokens[e] {
                Token::Exponent(s) => *s,
                _ => unreachable!("tokens[e] was previously matched as Token::Exponent"),
            };
            let digits = tokens[e + 1..]
                .iter()
                .filter(|t| matches!(t, Token::Digit(_)))
                .count();
            (true, sign, digits)
        }
        None => (false, ExpSign::Always, 0),
    };

    let percent_count = tokens
        .iter()
        .filter(|t| matches!(t, Token::Percent))
        .count();

    Desc {
        int_placeholders,
        frac_placeholders,
        exp_placeholders,
        use_grouping,
        scaling_commas,
        percent_count,
        has_exponent,
        exp_sign_style,
        int_placeholder_list,
        frac_placeholder_list,
    }
}

// ---------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------

fn render(abs_value: f64, prepend_minus: bool, tokens: &[Token], desc: &Desc) -> String {
    // Apply the value-level adjustments (percent scaling, scaling commas)
    // before splitting into integer / fractional / exponent parts.
    let mut v = abs_value;
    for _ in 0..desc.percent_count {
        v *= 100.0;
    }
    for _ in 0..desc.scaling_commas {
        v /= 1000.0;
    }

    let (int_str, frac_str, exp_str) = if desc.has_exponent {
        compute_scientific_parts(v, desc)
    } else {
        compute_decimal_parts(v, desc)
    };

    let dec_idx = tokens.iter().position(|t| matches!(t, Token::Decimal));
    let exp_idx = tokens.iter().position(|t| matches!(t, Token::Exponent(_)));
    let int_end = dec_idx.or(exp_idx).unwrap_or(tokens.len());
    let frac_end = exp_idx.unwrap_or(tokens.len());

    let mut out = String::new();
    if prepend_minus {
        out.push('-');
    }

    emit_integer_segment(&mut out, &tokens[..int_end], &int_str, desc);

    if let Some(d) = dec_idx {
        emit_fractional_segment(&mut out, &tokens[d..frac_end], &frac_str, desc);
    }

    if let Some(e) = exp_idx {
        emit_exponent_segment(&mut out, &tokens[e..], &exp_str);
    }

    out
}

fn compute_decimal_parts(v: f64, desc: &Desc) -> (String, String, String) {
    // Round with half-away-from-zero semantics (Rust's `.round()`),
    // which matches Excel's rounding in the common case. `format!`'s
    // default precision rounding is half-to-even in stable Rust and
    // would drift from Excel on `.5` boundaries.
    let rounded = if desc.frac_placeholders == 0 {
        v.round()
    } else {
        let mul = 10f64.powi(desc.frac_placeholders as i32);
        (v * mul).round() / mul
    };
    let safe = rounded.max(0.0); // belt-and-suspenders against -0.0
    let s = format!("{:.*}", desc.frac_placeholders, safe);
    let (int_part, frac_part) = match s.find('.') {
        Some(i) => (s[..i].to_string(), s[i + 1..].to_string()),
        None => (s, String::new()),
    };
    (int_part, frac_part, String::new())
}

fn compute_scientific_parts(v: f64, desc: &Desc) -> (String, String, String) {
    // Zero (and non-finite, as a defensive catch-all) short-circuit:
    // emit a zero mantissa and exp=0 so the output is stable.
    if v == 0.0 || !v.is_finite() {
        let int_part = "0".repeat(desc.int_placeholders.max(1));
        let frac_part = "0".repeat(desc.frac_placeholders);
        let exp_digits = "0".repeat(desc.exp_placeholders.max(1));
        let exp_sign = match desc.exp_sign_style {
            ExpSign::Always => "+",
            ExpSign::MinusOnly => "",
        };
        return (int_part, frac_part, format!("{}{}", exp_sign, exp_digits));
    }

    // Normalise so the mantissa integer part has exactly int_ph_eff digits.
    // `int_ph_eff` is clamped to 1 because `.00E+00` (no int placeholder)
    // is rare and still wants one mantissa digit before the decimal.
    let log = v.log10().floor() as i32;
    let int_ph_eff = desc.int_placeholders.max(1) as i32;
    let target_exp = log - (int_ph_eff - 1);
    let mantissa = v / 10f64.powi(target_exp);

    // Round mantissa to frac_placeholders decimal places with
    // half-away-from-zero.
    let mul = 10f64.powi(desc.frac_placeholders as i32);
    let rounded = (mantissa * mul).round() / mul;

    // Rounding can carry the mantissa past the bucket ceiling (9.99 →
    // 10.00 for 2-decimal, 1-int format). Re-normalise in that case.
    let max_mantissa = 10f64.powi(int_ph_eff);
    let (final_mantissa, final_exp) = if rounded >= max_mantissa {
        (rounded / 10.0, target_exp + 1)
    } else {
        (rounded, target_exp)
    };

    let mantissa_str = format!("{:.*}", desc.frac_placeholders, final_mantissa);
    let (int_part, frac_part) = match mantissa_str.find('.') {
        Some(i) => (
            mantissa_str[..i].to_string(),
            mantissa_str[i + 1..].to_string(),
        ),
        None => (mantissa_str, String::new()),
    };

    // Exponent: unsigned absolute value padded to exp_placeholders, with
    // a sign prefix per the E+/E- mode.
    let exp_abs = final_exp.unsigned_abs() as u64;
    let exp_digits_width = desc.exp_placeholders.max(1);
    let exp_digits = format!("{:0>width$}", exp_abs, width = exp_digits_width);
    let exp_sign_char = if final_exp < 0 {
        "-"
    } else {
        match desc.exp_sign_style {
            ExpSign::Always => "+",
            ExpSign::MinusOnly => "",
        }
    };
    let exp_part = format!("{}{}", exp_sign_char, exp_digits);
    (int_part, frac_part, exp_part)
}

fn emit_integer_segment(out: &mut String, int_tokens: &[Token], int_str: &str, desc: &Desc) {
    let placeholders = &desc.int_placeholder_list;
    let n_ph = placeholders.len();
    let digit_chars: Vec<char> = int_str.chars().collect();
    let n_d = digit_chars.len();

    // Build the raw digit sequence (pad / overflow handled but grouping
    // not yet applied). Leading pad chars come from placeholders with no
    // digit from `int_str`:
    //   * `0` → '0'
    //   * `?` → ' '
    //   * `#` → nothing (suppressed)
    let mut digit_seq: Vec<char> = Vec::new();
    if n_d >= n_ph {
        digit_seq.extend(digit_chars.iter().copied());
    } else {
        let pad_count = n_ph - n_d;
        for ph in placeholders.iter().take(pad_count) {
            match ph {
                DigitPlace::Required => digit_seq.push('0'),
                DigitPlace::Space => digit_seq.push(' '),
                DigitPlace::Optional => {} // suppress — no char
            }
        }
        digit_seq.extend(digit_chars.iter().copied());
    }

    // POI leading-zero suppression: if the value rounds to zero (v < 1)
    // and the **leading placeholder for the value digit** is `#`, drop
    // the single leading '0' we'd otherwise emit. Examples: `#.##` for
    // 0.5 renders ".5", not "0.5".
    //
    // The rule only applies when `int_str` (length 1, value "0") fits
    // into the placeholder block via the overflow path — i.e. there are
    // no leading Optional pad slots ahead of the digit. For
    // `#,##0.00` rendering 0, the '0' lands in the rightmost Required
    // placeholder and must be preserved (Excel renders "0.00", not
    // ".00"). So we gate on `n_d >= n_ph`, which forces n_ph ∈ {0, 1}.
    let strip_leading_zero = n_d >= n_ph
        && int_str == "0"
        && placeholders.first() == Some(&DigitPlace::Optional)
        && digit_seq.first() == Some(&'0');
    if strip_leading_zero {
        digit_seq.remove(0);
    }

    let grouped_string = if desc.use_grouping {
        apply_grouping(&digit_seq)
    } else {
        digit_seq.iter().collect::<String>()
    };

    // Walk tokens; the digit block lands once (at the first Digit token)
    // and subsequent Digit / Group tokens are no-ops. Literals and
    // padding render in position so `+0"bps"` and `$#,##0` work.
    let mut emitted_digit_block = false;
    for tok in int_tokens {
        match tok {
            Token::Digit(_) if !emitted_digit_block => {
                out.push_str(&grouped_string);
                emitted_digit_block = true;
            }
            Token::Literal(s) => out.push_str(s),
            Token::Pad => out.push(' '),
            Token::Percent => out.push('%'),
            // Token::Digit after the block is emitted, Token::Group
            // (folded into grouped_string), Token::Fill (dropped), and
            // Decimal/Exponent (shouldn't land here) are all no-ops.
            _ => {}
        }
    }
}

/// Insert thousand separators into a contiguous run of digits, preserving
/// any leading whitespace (from `?`-pad placeholders) verbatim.
fn apply_grouping(digits: &[char]) -> String {
    let lead_end = digits
        .iter()
        .position(|c| c.is_ascii_digit())
        .unwrap_or(digits.len());
    let lead: String = digits[..lead_end].iter().collect();
    let digs: Vec<char> = digits[lead_end..].to_vec();

    if digs.len() <= 3 {
        return format!("{}{}", lead, digs.iter().collect::<String>());
    }

    let mut rev_buf: Vec<char> = Vec::with_capacity(digs.len() + digs.len() / 3);
    for (count, c) in digs.iter().rev().enumerate() {
        if count > 0 && count % 3 == 0 {
            rev_buf.push(',');
        }
        rev_buf.push(*c);
    }
    rev_buf.reverse();
    format!("{}{}", lead, rev_buf.iter().collect::<String>())
}

fn emit_fractional_segment(out: &mut String, frac_tokens: &[Token], frac_str: &str, desc: &Desc) {
    let placeholders = &desc.frac_placeholder_list;
    let digit_chars: Vec<char> = frac_str.chars().collect();

    // Pair placeholders with frac digits left-to-right. `frac_str` is
    // zero-padded to `placeholders.len()` by `compute_decimal_parts`, so
    // the zip is exact; `unwrap_or('0')` is defensive.
    //
    // `?` in the fractional part emits the digit when it's non-zero, and
    // a space when it's zero (POI's "column-alignment" use-case). Real
    // Excel renders `?` as a fixed-width placeholder even for non-zero
    // digits; we preserve the digit so round-trip snapshots stay
    // meaningful.
    let mut frac_digits: Vec<char> = Vec::new();
    for (i, ph) in placeholders.iter().enumerate() {
        let c = digit_chars.get(i).copied().unwrap_or('0');
        frac_digits.push(match ph {
            DigitPlace::Required | DigitPlace::Optional => c,
            DigitPlace::Space => {
                if c == '0' {
                    ' '
                } else {
                    c
                }
            }
        });
    }

    // Trim trailing '0' digits whose placeholder is Optional (`#`). Stop
    // at the first non-'0' or non-Optional position.
    while let Some(idx) = frac_digits.len().checked_sub(1)
        && frac_digits[idx] == '0'
        && matches!(placeholders.get(idx), Some(DigitPlace::Optional))
    {
        frac_digits.pop();
    }

    let any_frac = !frac_digits.is_empty();
    // If every fractional placeholder got trimmed AND there are no
    // literals in the fractional section, suppress the decimal point
    // too — otherwise we'd emit `.` hanging at the end of the integer
    // part, which is Excel's behaviour only when a literal demands it.
    let frac_has_literal_after_dot = frac_tokens
        .iter()
        .skip(1)
        .any(|t| matches!(t, Token::Literal(_) | Token::Pad));
    let emit_decimal_point = any_frac || frac_has_literal_after_dot;

    let mut emitted_digit_block = false;
    for tok in frac_tokens {
        match tok {
            Token::Decimal if emit_decimal_point => out.push('.'),
            Token::Digit(_) if !emitted_digit_block => {
                out.extend(frac_digits.iter());
                emitted_digit_block = true;
            }
            Token::Literal(s) => out.push_str(s),
            Token::Pad => out.push(' '),
            Token::Percent => out.push('%'),
            // Decimal when suppressed, repeat Digit after the block,
            // Group (shouldn't appear in frac), and Fill (dropped): all
            // no-ops.
            _ => {}
        }
    }
}

fn emit_exponent_segment(out: &mut String, exp_tokens: &[Token], exp_str: &str) {
    // `exp_tokens[0]` is the Exponent marker; emit `E` then the pre-
    // formatted signed-and-padded exponent string. Subsequent Digit
    // tokens are the placeholder run whose width already drove
    // `exp_str`'s padding, so they're no-ops.
    let mut emitted_exp_block = false;
    for tok in exp_tokens {
        match tok {
            Token::Exponent(_) => {
                out.push('E');
                if !emitted_exp_block {
                    out.push_str(exp_str);
                    emitted_exp_block = true;
                }
            }
            Token::Digit(_) => {
                // Already reflected in `exp_str`'s zero-padding.
            }
            Token::Literal(s) => out.push_str(s),
            Token::Pad => out.push(' '),
            Token::Fill => {}
            Token::Percent => out.push('%'),
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Fraction renderer
// ---------------------------------------------------------------------------
//
// Triggered whenever the tokenizer produced a `Token::Slash`. The layout
// breaks the token stream into four ordered segments:
//
//   tokens[..num_token_start]                — integer segment
//   tokens[num_token_start..slash_token]     — numerator placeholder run
//   tokens[slash_token]                      — Slash
//   tokens[slash_token+1..suffix_token_start]— denominator (Fixed or Auto)
//   tokens[suffix_token_start..]             — suffix literals (e.g. `"`)
//
// Rendering computes the best rational approximation of `abs_value`'s
// fractional part, handles carry (e.g. `96.9375` with `?/8` → `97`),
// and walks the layout once to emit whole / separator / numerator /
// `/` / denominator / suffix in place. This matches Excel's on-screen
// rendering for the common engineering / spec formats (`# ?/4`,
// `# ?/8`, `# ?/16`, `# ?/?`, `# ??/??`).

/// Denominator specification extracted from the format body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DenomSpec {
    /// `?/8`, `?/16`, `?/100` — exact literal denominator. Excel and
    /// LibreOffice both reduce the resulting fraction via GCD (OpenXML
    /// convention), so `0.25` against `?/8` renders `1/4`, not `2/8`.
    /// The placeholder count caps the numerator width; the rendered
    /// denominator is the canonical reduced form.
    Fixed(u32),
    /// `?/?`, `?/??`, `?/???` — auto-denominator. The usize is the
    /// placeholder count (`K`) which bounds the denominator at
    /// `10^K - 1`. We pick the best rational approximation within that
    /// bound, naturally yielding reduced forms for representable values
    /// (e.g. `0.5` → `1/2`, not `5/10`).
    Auto(usize),
}

/// Ordered token-stream boundaries for a fraction body.
///
/// Borrow-friendly slice indices: pair with the `&[Token]` passed into
/// [`render_fraction`] to walk each segment without reallocating.
#[derive(Debug, Clone, Copy)]
struct FractionLayout {
    /// Index of the first token of the numerator placeholder run.
    /// Also the exclusive end of the integer segment.
    num_token_start: usize,
    /// Index of the `Token::Slash` itself.
    slash_token: usize,
    /// Exclusive end of the denominator token(s). For `DenomSpec::Fixed`
    /// this is `slash_token + 2` (the `FixedDenom` follows the Slash);
    /// for `DenomSpec::Auto(k)` it's `slash_token + 1 + k`.
    suffix_token_start: usize,
    denom_spec: DenomSpec,
}

/// Identify the fraction layout for a token stream containing a
/// `Token::Slash`. Returns `None` for ill-formed fraction bodies —
/// missing numerator placeholders, missing denominator, zero
/// denominator — so `format_number_body` can raise a structured
/// [`FormatWarningKind::FormatFeatureFraction`] warning.
fn detect_fraction(tokens: &[Token]) -> Option<FractionLayout> {
    let slash_token = tokens.iter().position(|t| matches!(t, Token::Slash))?;

    // Numerator: consecutive Digit placeholders immediately preceding
    // the Slash. At least one placeholder required — a bare `/8` with
    // no numerator is not a valid fraction format.
    let mut num_token_start = slash_token;
    while num_token_start > 0 && matches!(tokens[num_token_start - 1], Token::Digit(_)) {
        num_token_start -= 1;
    }
    if num_token_start == slash_token {
        return None;
    }

    // Denominator: a FixedDenom token pushed by the tokenizer (the `8`
    // in `?/8`) or a run of Digit placeholders (the `?` in `?/?`).
    let denom_start = slash_token + 1;
    let (denom_spec, denom_end) = match tokens.get(denom_start)? {
        Token::FixedDenom(n) if *n > 0 => (DenomSpec::Fixed(*n), denom_start + 1),
        Token::Digit(_) => {
            let mut end = denom_start;
            while end < tokens.len() && matches!(tokens[end], Token::Digit(_)) {
                end += 1;
            }
            let count = end - denom_start;
            (DenomSpec::Auto(count), end)
        }
        _ => return None,
    };

    Some(FractionLayout {
        num_token_start,
        slash_token,
        suffix_token_start: denom_end,
        denom_spec,
    })
}

/// Euclidean GCD on non-negative `i64`. Used by the Fixed-denominator
/// branch to reduce the rendered fraction (Excel / LibreOffice
/// convention). `gcd(0, n) == n` and `gcd(n, 0) == n` per the standard
/// definition; the result is clamped to ≥ 1 so the divisor in the
/// reduction step is always safe.
fn gcd_i64(a: i64, b: i64) -> i64 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a.max(1)
}

/// Compute `(numerator, denominator, carry)` for a non-negative
/// fractional part against a denominator spec.
///
/// `carry` is `1` iff rounding pushed the numerator past the
/// denominator (the whole-number part should be incremented by 1) or
/// the auto-denominator search found no representable non-trivial
/// fraction closer to the value than rounding up to the next integer.
/// `carry` is `0` for the in-bounds case — including the "drop the
/// fraction" case where the value is closer to zero than to any
/// representable fraction.
fn compute_fraction_parts(frac: f64, spec: DenomSpec) -> (i64, i64, i64) {
    let frac = frac.abs();
    if frac == 0.0 {
        return (0, 1, 0);
    }
    match spec {
        DenomSpec::Fixed(d) => {
            let d = d as i64;
            let n = (frac * d as f64).round() as i64;
            if n >= d {
                (0, d, 1)
            } else if n == 0 {
                // Rounded to zero — no fractional part, no carry.
                (0, d, 0)
            } else {
                // Excel and LibreOffice both reduce fixed-denominator
                // fractions to their canonical form: `0.25` with `?/8`
                // renders `1/4`, not `2/8`. The rounded numerator and
                // literal denominator share a GCD ≥ 1; dividing both
                // by it yields the reduced pair.
                let g = gcd_i64(n, d);
                (n / g, d / g, 0)
            }
        }
        DenomSpec::Auto(places) => {
            // 10^K - 1 caps the denominator: `?/?` → 9, `?/??` → 99.
            // `pow` on i64 is safe here because `places` is driven by
            // placeholder count (realistically ≤ 3).
            let max_denom = 10i64
                .checked_pow(places as u32)
                .map(|p| p.saturating_sub(1))
                .unwrap_or(1)
                .max(1);

            // Baseline: drop the fraction (err = frac itself).
            let mut best_num = 0i64;
            let mut best_denom = 1i64;
            let mut best_carry = 0i64;
            let mut best_err = frac;

            // Carry baseline: round the whole number up.
            let carry_err = 1.0 - frac;
            if carry_err < best_err {
                best_err = carry_err;
                best_num = 0;
                best_denom = 1;
                best_carry = 1;
            }

            // Brute-force best rational with denom ≤ max_denom. Realistic
            // max_denom is ≤ 999 so the outer loop is trivial; smaller
            // denominators win ties because they're visited first.
            for d in 1..=max_denom {
                let n = (frac * d as f64).round() as i64;
                if n <= 0 || n >= d {
                    continue;
                }
                let approx = n as f64 / d as f64;
                let err = (frac - approx).abs();
                if err < best_err {
                    best_err = err;
                    best_num = n;
                    best_denom = d;
                    best_carry = 0;
                }
            }

            (best_num, best_denom, best_carry)
        }
    }
}

/// Render a fraction-format body against an absolute numeric value.
fn render_fraction(
    abs_value: f64,
    prepend_minus: bool,
    tokens: &[Token],
    layout: &FractionLayout,
) -> String {
    // Excel truncates toward zero to get the whole part, then computes
    // the sub-unit fraction. Rounding-induced carry (e.g. `96.9375`
    // with `?/8` → whole=97, num=0) is absorbed via the `carry` return.
    let whole_abs = abs_value.trunc() as i64;
    let frac = abs_value - whole_abs as f64;
    let (num, denom, carry) = compute_fraction_parts(frac, layout.denom_spec);
    let whole = whole_abs + carry;

    let mut out = String::new();
    if prepend_minus {
        out.push('-');
    }

    let int_placeholders: Vec<DigitPlace> = tokens[..layout.num_token_start]
        .iter()
        .filter_map(|t| match t {
            Token::Digit(p) => Some(*p),
            _ => None,
        })
        .collect();
    let int_has_required = int_placeholders
        .iter()
        .any(|p| matches!(p, DigitPlace::Required));

    // Display rules, cribbed from POI/Excel behaviour:
    //   * No fractional part (num == 0) → always emit the whole.
    //   * Whole == 0 and the integer placeholder is all `#` → suppress
    //     the leading zero (consistent with `#.##` dropping the leading
    //     zero on `0.5` → `.5`).
    //   * `0`-placeholder forces a whole of `0` to appear even when a
    //     fractional part follows (`0 ?/?` on `0.5` → `0 1/2`).
    let show_fraction = num != 0;
    let show_whole = whole != 0 || int_has_required || !show_fraction;

    if show_whole {
        let whole_str = whole.to_string();
        // Determine how much of the integer-segment token run to walk:
        // when a fraction follows, include the separator literals
        // between the last integer placeholder and the numerator (the
        // ` ` in `# ?/8`); otherwise trim to the last integer digit so
        // a trailing separator doesn't leak into a purely-whole render.
        let last_int_digit_pos = tokens[..layout.num_token_start]
            .iter()
            .rposition(|t| matches!(t, Token::Digit(_)));
        let int_walk_end = match (show_fraction, last_int_digit_pos) {
            (true, _) => layout.num_token_start,
            (false, Some(pos)) => pos + 1,
            (false, None) => 0,
        };
        if int_walk_end == 0 && last_int_digit_pos.is_none() {
            // No integer placeholder in the format (e.g. `?/?` only);
            // emit the whole inline so values like `96.125` render as
            // `96 1/8` rather than dropping the whole part entirely.
            out.push_str(&whole_str);
            if show_fraction {
                out.push(' ');
            }
        } else {
            let mut emitted_whole = false;
            for tok in &tokens[..int_walk_end] {
                match tok {
                    Token::Digit(_) if !emitted_whole => {
                        out.push_str(&whole_str);
                        emitted_whole = true;
                    }
                    Token::Literal(s) => out.push_str(s),
                    Token::Pad => out.push(' '),
                    _ => {}
                }
            }
        }
    }

    if show_fraction {
        // Numerator block — placeholder run between integer segment and
        // the Slash. Any interleaved literals (rare) render in position.
        let num_str = num.to_string();
        let mut emitted_num = false;
        for tok in &tokens[layout.num_token_start..layout.slash_token] {
            match tok {
                Token::Digit(_) if !emitted_num => {
                    out.push_str(&num_str);
                    emitted_num = true;
                }
                Token::Literal(s) => out.push_str(s),
                _ => {}
            }
        }
        out.push('/');
        out.push_str(&denom.to_string());
    }

    // Suffix literals — whatever follows the denominator (e.g. the
    // inch-mark `"` in `# ?/8\"`, or the " sq. ft." suffix on an area
    // format).
    for tok in &tokens[layout.suffix_token_start..] {
        match tok {
            Token::Literal(s) => out.push_str(s),
            Token::Pad => out.push(' '),
            _ => {}
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(body: &str, value: f64) -> String {
        format_number_body(value.abs(), value < 0.0, body).expect("body should render")
    }

    fn fmt_abs(body: &str, abs_value: f64, prepend_minus: bool) -> String {
        format_number_body(abs_value, prepend_minus, body).expect("body should render")
    }

    // --- Plain integers ----------------------------------------------------

    #[test]
    fn plain_integer_no_padding() {
        assert_eq!(fmt("0", 42.0), "42");
        assert_eq!(fmt("0", 0.0), "0");
    }

    #[test]
    fn plain_integer_zero_pad() {
        assert_eq!(fmt("000", 42.0), "042");
        assert_eq!(fmt("00000", 3.0), "00003");
    }

    #[test]
    fn plain_integer_optional_no_pad() {
        assert_eq!(fmt("#####", 42.0), "42");
    }

    #[test]
    fn plain_integer_negative_prepends_minus() {
        assert_eq!(fmt("0", -42.0), "-42");
        assert_eq!(fmt("000", -5.0), "-005");
    }

    // --- Decimals -----------------------------------------------------------

    #[test]
    fn two_decimals_required() {
        assert_eq!(fmt("0.00", 1.5), "1.50");
        assert_eq!(fmt("0.00", 1.0), "1.00");
        assert_eq!(fmt("0.00", 0.0), "0.00");
    }

    #[test]
    fn rounds_half_away_from_zero() {
        // 2.918 rounded to 2 decimals → 2.92 (the watchlist canary).
        assert_eq!(fmt("0.00", 2.918), "2.92");
        // Round-up carries into the integer part cleanly.
        assert_eq!(fmt("0.0", 0.95), "1.0");
    }

    #[test]
    fn optional_trailing_zero_suppressed() {
        assert_eq!(fmt("0.0#", 1.0), "1.0");
        assert_eq!(fmt("0.0#", 1.2), "1.2");
        assert_eq!(fmt("0.0#", 1.23), "1.23");
    }

    #[test]
    fn decimal_dropped_when_all_fractional_optional_and_zero() {
        // `0.##` renders an integer as `"1"`, not `"1."`.
        assert_eq!(fmt("0.##", 1.0), "1");
        assert_eq!(fmt("0.##", 1.5), "1.5");
    }

    #[test]
    fn percent_multiplies_by_hundred() {
        assert_eq!(fmt("0%", 0.42), "42%");
        assert_eq!(fmt("0.0%", 0.42), "42.0%");
        // The original calamine-era regression: -2.6% on a -0.0261111
        // ratio. Passes abs + prepend_minus via `fmt`.
        assert_eq!(fmt("0.0%", -0.0261111), "-2.6%");
    }

    // --- Grouping and scaling ----------------------------------------------

    #[test]
    fn grouping_comma_inserts_thousands_separator() {
        assert_eq!(fmt("#,##0", 1234.0), "1,234");
        assert_eq!(fmt("#,##0", 1_234_567.0), "1,234,567");
        assert_eq!(fmt("#,##0", 42.0), "42");
    }

    #[test]
    fn grouping_plus_decimals() {
        assert_eq!(fmt("#,##0.00", 1234.5), "1,234.50");
        assert_eq!(fmt("#,##0.00", 0.0), "0.00");
    }

    #[test]
    fn trailing_comma_scales_by_thousand() {
        // `#,##0,` divides by 1000 before rendering.
        assert_eq!(fmt("#,##0,", 1_000_000.0), "1,000");
        assert_eq!(fmt("#,##0,,", 1_000_000_000.0), "1,000");
    }

    // --- Scientific --------------------------------------------------------

    #[test]
    fn watchlist_scientific_format_renders_correctly() {
        // The exact defect: `0.00E+00` on 2.918e13 used to render
        // `29180000000000.00`. POI-aligned: `2.92E+13`.
        assert_eq!(fmt("0.00E+00", 2.918e13), "2.92E+13");
    }

    #[test]
    fn scientific_small_value_renders_negative_exponent() {
        assert_eq!(fmt("0.00E+00", 5.2e-5), "5.20E-05");
    }

    #[test]
    fn scientific_minus_only_sign_suppresses_positive_plus() {
        assert_eq!(fmt("0.00E-00", 2.918e13), "2.92E13");
        assert_eq!(fmt("0.00E-00", 5.2e-5), "5.20E-05");
    }

    #[test]
    fn scientific_rounds_carries_exponent() {
        // 9.999e3 with `0.00E+00` rounds mantissa to 10.00 → carry to
        // 1.00E+04.
        assert_eq!(fmt("0.00E+00", 9.999e3), "1.00E+04");
    }

    #[test]
    fn scientific_zero_stable() {
        assert_eq!(fmt("0.00E+00", 0.0), "0.00E+00");
    }

    // --- Literals ----------------------------------------------------------

    #[test]
    fn currency_literal_prefix() {
        assert_eq!(fmt("$#,##0", 450_000.0), "$450,000");
    }

    #[test]
    fn watchlist_bps_positive_section() {
        // `+0"bps"` (the positive section body of the watchlist format).
        // has_sign is `false` for this section (the `+` doesn't count),
        // so the engine-level caller passes prepend_minus=false for
        // positive input, and the body's literal `+` renders verbatim.
        assert_eq!(fmt_abs("+0\"bps\"", 1.0, false), "+1bps");
    }

    #[test]
    fn watchlist_bps_negative_section_renders_single_minus() {
        // `-0"bps"` negative body: the section layer detects has_sign=true,
        // so the caller passes abs(value) and prepend_minus=false. The
        // body's literal `-` produces the sign. This is the fix for the
        // `-+1bps` double-sign defect.
        assert_eq!(fmt_abs("-0\"bps\"", 1.0, false), "-1bps");
    }

    #[test]
    fn literal_x_suffix() {
        assert_eq!(fmt("0.0\"x\"", 62.4), "62.4x");
        assert_eq!(fmt("0.0\"x\"", -62.4), "-62.4x");
    }

    #[test]
    fn backslash_escape_emits_literal() {
        assert_eq!(fmt(r"0\X", 5.0), "5X");
    }

    #[test]
    fn underscore_pad_emits_single_space() {
        assert_eq!(fmt("0_)", 5.0), "5 ");
    }

    #[test]
    fn star_fill_is_dropped() {
        assert_eq!(fmt("0*-", 5.0), "5");
    }

    // --- Fractions ---------------------------------------------------------

    #[test]
    fn fraction_fixed_denom_renders_mixed_form() {
        // The manifest defect: `96.125` with `# ?/8"` (bare trailing
        // inch-mark) must render as `96 1/8"`, not fall back to raw
        // decimal. The lenient tokenizer treats the unclosed `"` as a
        // literal suffix, which is what Excel does in practice.
        // `96.250` reduces to `96 1/4"` per the OpenXML reduction
        // convention (Excel + LibreOffice both reduce via GCD).
        assert_eq!(fmt(r#"# ?/8""#, 96.125), r#"96 1/8""#);
        assert_eq!(fmt(r#"# ?/8""#, 96.250), r#"96 1/4""#);
        assert_eq!(fmt(r#"# ?/8""#, 96.375), r#"96 3/8""#);
    }

    #[test]
    fn fraction_fixed_denom_whole_only_suppresses_fraction() {
        // `96.000` with `?/8` rounds the fractional part to 0/8. Excel
        // emits only the whole number; the numerator slot collapses.
        assert_eq!(fmt(r#"# ?/8""#, 96.0), r#"96""#);
    }

    #[test]
    fn fraction_fixed_denom_rounding_carries_to_whole() {
        // `96.9375` with `?/8` rounds to 8/8 → carry to 97 with no
        // fractional part.
        assert_eq!(fmt(r#"# ?/8""#, 96.9375), r#"97""#);
    }

    #[test]
    fn fraction_fixed_denom_is_reduced_to_canonical_form() {
        // Excel and LibreOffice both reduce fixed-denominator fractions
        // via GCD (OpenXML convention): `0.25` with `?/8` renders
        // `1/4`, not `2/8`. With a leading `#` the zero whole is
        // suppressed. `0.5` with `?/8` reduces `4/8` → `1/2`.
        assert_eq!(fmt("# ?/8", 0.25), "1/4");
        assert_eq!(fmt("# ?/8", 0.5), "1/2");
        // Non-reducible numerators stay literal — `0.125` → `1/8`.
        assert_eq!(fmt("# ?/8", 0.125), "1/8");
    }

    #[test]
    fn fraction_auto_denom_picks_best_rational() {
        // `# ?/?` bounds the denom at 9. `0.5` → `1/2`, `0.333…` → `1/3`.
        // Leading `#` suppresses the zero whole.
        assert_eq!(fmt("# ?/?", 0.5), "1/2");
        assert_eq!(fmt("# ?/?", 1.0 / 3.0), "1/3");
        assert_eq!(fmt("# ?/?", 2.5), "2 1/2");
    }

    #[test]
    fn fraction_auto_two_digit_denom_improves_precision() {
        // `??/??` unlocks denominators up to 99. For π's fractional part
        // (0.14159…), 14/99 ≈ 0.1414 is closer than any other rational
        // with denom ≤ 99, so brute-force picks `3 14/99`.
        let s = fmt("# ??/??", std::f64::consts::PI);
        assert_eq!(s, "3 14/99");
    }

    #[test]
    fn fraction_negative_value_prepends_minus() {
        assert_eq!(fmt(r#"# ?/8""#, -96.125), r#"-96 1/8""#);
    }

    #[test]
    fn fraction_zero_value_with_required_whole() {
        // `0 ?/?` forces the `0` whole to render even when there is no
        // fractional part.
        assert_eq!(fmt("0 ?/?", 0.0), "0");
    }

    #[test]
    fn fraction_improper_no_whole_placeholder_keeps_integer_inline() {
        // `?/?` with no integer placeholder still renders the whole part
        // inline (Excel behaviour) — dropping it would silently mangle
        // values ≥ 1.
        assert_eq!(fmt("?/?", 2.5), "2 1/2");
    }

    // --- Out-of-scope grammar raises warnings ------------------------------

    #[test]
    fn fraction_without_numerator_placeholder_returns_warning() {
        // `/8` has no numerator placeholder → malformed per the grammar.
        let err = format_number_body(0.5, false, "/8").unwrap_err();
        assert_eq!(err.kind, FormatWarningKind::FormatFeatureFraction);
    }

    #[test]
    fn unterminated_quoted_literal_renders_leniently_as_literal() {
        // POI's regex-based tokenizer skips unmatched `"`. We mirror
        // that: `0"bps` renders the `"bps` as a literal suffix rather
        // than failing the whole cell, so a forgotten closing quote
        // doesn't silently drop a column's formatting.
        assert_eq!(fmt("0\"bps", 1.0), "1\"bps");
        // Bare trailing `"` — the manifest.xlsx inch-mark case — lands
        // as a single literal char.
        assert_eq!(fmt("0\"", 5.0), "5\"");
    }

    #[test]
    fn trailing_backslash_returns_malformed() {
        let err = format_number_body(1.0, false, "0\\").unwrap_err();
        assert_eq!(err.kind, FormatWarningKind::MalformedFormatCode);
    }

    // --- Sign interaction with the section layer --------------------------

    #[test]
    fn prepend_minus_applied_when_body_has_no_sign() {
        // Single section `0.00`, negative value: section layer sets
        // has_sign=false and prepend_minus=true.
        assert_eq!(fmt_abs("0.00", 1.0, true), "-1.00");
    }

    #[test]
    fn prepend_minus_not_applied_when_body_has_its_own_sign() {
        // Mimics the POI dispatch for `0.00;(0.00)` negative body with
        // value -5: has_sign=true (parens), prepend_minus=false, body
        // renders `(5.00)`.
        assert_eq!(fmt_abs("(0.00)", 5.0, false), "(5.00)");
    }
}
