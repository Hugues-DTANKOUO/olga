//! Section splitter + `CellFormatPart` parser for Excel number formats.
//!
//! The parsing strategy and section semantics are informed by Apache POI's
//! `CellFormatPart` (see ATTRIBUTION.md): POI is the most legible reference
//! implementation of Excel's format grammar. Olga's Rust implementation is
//! independent and adapted to our primitive model.
//!
//! # Why this lives in its own module
//!
//! Excel's number-format grammar is a mini-language with four layers:
//!
//!   1. **Section split.** A format code is up to four sections separated
//!      by `;` — applied to positive / negative / zero / text values.
//!      Separators inside `"..."` or `[...]` don't count.
//!
//!   2. **Prefix strip.** Each section may open with up to three
//!      bracketed prefixes in any order: a condition (`[>=100]`), a
//!      colour (`[Red]`, `[Color 12]`), and a locale / currency
//!      (`[$-409]`, `[$$-409]`). After consuming them the body remains.
//!
//!   3. **Kind detection.** The body is classified as Date, Elapsed,
//!      Text, or Number. Elapsed (`[h]`, `[m]`, `[s]`) is its own kind
//!      because it consumes a date-serial numerically, not calendrically.
//!
//!   4. **`hasSign()`.** If a non-positive section contains a literal
//!      `-`, `(`, or `)` *outside quotes*, the caller pre-negates the
//!      value (passes `abs(value)` to the body formatter). Without this,
//!      the engine emits its own minus *and* the section's literal `-`
//!      for a double sign (the `-+1bps` defect on watchlist.xlsx); the
//!      same rule is applied by POI's formatter.
//!
//! Keeping the section semantics tight at this layer means each future
//! formatter (`CellNumberFormatter`, `CellDateFormatter`, etc.) receives
//! a body that is already signed-correct, prefix-stripped, and kind-tagged
//! — so the formatters themselves stay focused on body-specific grammar.

use super::contract::{FormatWarning, FormatWarningKind};

/// A single `;`-delimited section after POI-style decomposition.
///
/// The `body` is what the per-kind formatter consumes: bracket prefixes
/// have been stripped, sign handling is described by `has_sign`, and
/// kind classification is complete.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct FormatSection {
    /// Verbatim section body with bracket prefixes removed. `mm/dd/yyyy`
    /// or `+0"bps"`, not `[Red]+0"bps"`.
    pub(super) body: String,

    /// Condition like `[>=100]` that gates which values this section
    /// applies to. POI treats unconditioned sections as falling through
    /// in positional order.
    pub(super) condition: Option<Condition>,

    /// Excel colour code (`[Red]`, `[Blue]`, `[Color 12]`). Stored as
    /// a lowercase token — we don't emit styling, but downstream
    /// consumers may want to preserve it.
    pub(super) color: Option<String>,

    /// Numeric locale hint from a `[$-XXX]` or `[$SYMBOL-XXX]` prefix.
    /// The hex locale id is stored as a string so later stages can
    /// decide locale-sensitive rules (month names, grouping separator).
    pub(super) locale: Option<String>,

    /// Currency glyph from a `[$SYMBOL-XXX]` prefix (`$`, `€`, …).
    /// Distinct from a quoted `"$"` literal because the locale context
    /// differs; POI keeps the same split.
    pub(super) currency_symbol: Option<String>,

    /// Kind of body — drives which formatter picks this section up.
    pub(super) kind: SectionKind,

    /// POI's `hasSign()` — true iff the body (outside quotes/escapes)
    /// contains a literal `-`, `(`, or `)`. If set, the engine MUST
    /// pre-negate the value (pass `abs(value)` to the body formatter)
    /// and MUST NOT prepend its own minus sign. This is what eliminates
    /// the `-+1bps` / `(-$5.00)` double-sign defects.
    pub(super) has_sign: bool,
}

/// Kind of a section body, POI-aligned.
///
/// `General` is separate from `Number` because POI's
/// `CellGeneralFormatter` has its own rules (up to 11 significant
/// digits, auto-switch to scientific beyond ±1e16) that differ from
/// the `0`/`#`/`?` placeholder machinery in `CellNumberFormatter`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SectionKind {
    /// Explicit `General` keyword.
    General,
    /// `0`/`#`/`?`-driven number, possibly with `E+/E-`, `%`, decimals,
    /// grouping commas, literals.
    Number,
    /// Calendrical date/time tokens (`y`, `m`, `d`, `h`, `s`, `am/pm`).
    Date,
    /// Elapsed-time markers (`[h]`, `[m]`, `[s]`) — duration, not
    /// calendar. Stays in Elapsed even if calendrical tokens coexist.
    Elapsed,
    /// `@` placeholder — echo the original text verbatim.
    Text,
}

/// Conditional predicate gating a section.
///
/// Matches POI's six comparators on a single decimal literal. The
/// value side is always an `f64` because Excel stores everything as
/// a double — conditions against the raw number, not the rendered
/// string.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Condition {
    LessThan(f64),
    LessOrEqual(f64),
    Equal(f64),
    NotEqual(f64),
    GreaterOrEqual(f64),
    GreaterThan(f64),
}

impl Condition {
    /// Evaluate the condition against a numeric value.
    #[allow(dead_code)] // consumed by select_section in a follow-up wiring pass
    pub(super) fn matches(&self, value: f64) -> bool {
        match *self {
            Condition::LessThan(t) => value < t,
            Condition::LessOrEqual(t) => value <= t,
            Condition::Equal(t) => (value - t).abs() < f64::EPSILON,
            Condition::NotEqual(t) => (value - t).abs() >= f64::EPSILON,
            Condition::GreaterOrEqual(t) => value >= t,
            Condition::GreaterThan(t) => value > t,
        }
    }
}

/// Parsed form of a full format code: ordered list of sections.
///
/// Length is between 1 and 4; the splitter drops empty trailing
/// sections but keeps empty *leading* sections (`;;;@` has three empty
/// sections followed by a text section — Excel uses this to hide
/// numeric cells).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ParsedFormat {
    pub(super) sections: Vec<FormatSection>,
}

/// Split a format code into sections and parse each one.
///
/// Errors only on syntactically broken codes (unterminated literal,
/// unterminated bracket). Every other failure yields a best-effort
/// parse — the degradation contract deliberately prefers fallback
/// plus warning over refusal.
#[allow(dead_code)] // wired into format_value by #40/#41/#42
pub(super) fn split_format(code: &str) -> Result<ParsedFormat, FormatWarning> {
    let raw_sections = split_sections(code)?;
    let mut sections = Vec::with_capacity(raw_sections.len());
    for raw in raw_sections {
        sections.push(parse_part(&raw)?);
    }
    Ok(ParsedFormat { sections })
}

/// Choose the section that applies to a given value, POI `selectFormat`
/// semantics.
///
/// Rules:
///
/// * If any section has a condition, the first section whose condition
///   matches wins. POI evaluates conditions in position order.
/// * Otherwise, fall through by section count:
///   * 1 section — always that one.
///   * 2 sections — `pos+zero` then `neg`.
///   * 3 sections — `pos` then `neg` then `zero`.
///   * 4 sections — `pos` then `neg` then `zero` then `text` (text
///     section is not selected for numeric values; caller handles that
///     branch separately).
///
/// Returns the index into `parsed.sections`; callers use it to look up
/// the `has_sign` flag and the body.
#[allow(dead_code)] // wired into format_value by #40/#41/#42
pub(super) fn select_section_index(parsed: &ParsedFormat, value: f64) -> usize {
    let conditioned: Vec<(usize, &Condition)> = parsed
        .sections
        .iter()
        .enumerate()
        .filter_map(|(i, s)| s.condition.as_ref().map(|c| (i, c)))
        .collect();
    if !conditioned.is_empty() {
        for (idx, cond) in &conditioned {
            if cond.matches(value) {
                return *idx;
            }
        }
        // POI falls back to the last *unconditioned* section when no
        // condition matches. If all are conditioned, POI picks the
        // last section (matches the spec's "default" slot).
        if let Some((idx, _)) = parsed
            .sections
            .iter()
            .enumerate()
            .rev()
            .find(|(_, s)| s.condition.is_none())
        {
            return idx;
        }
        return parsed.sections.len() - 1;
    }

    match parsed.sections.len() {
        0 | 1 => 0,
        2 => {
            if value < 0.0 {
                1
            } else {
                0
            }
        }
        _ => {
            if value > 0.0 {
                0
            } else if value < 0.0 {
                1
            } else {
                2
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Section splitter
// ---------------------------------------------------------------------------

/// Walk the format code and split on `;` at depth-0, honoring quoted
/// runs, bracketed tokens, and backslash escapes.
fn split_sections(code: &str) -> Result<Vec<String>, FormatWarning> {
    let mut out = Vec::with_capacity(4);
    let mut buf = String::new();
    let mut chars = code.chars().peekable();
    let mut in_quotes = false;
    let mut bracket_depth = 0u32;

    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
                buf.push(c);
            }
            '\\' if !in_quotes => {
                buf.push(c);
                if let Some(next) = chars.next() {
                    buf.push(next);
                }
            }
            '[' if !in_quotes => {
                bracket_depth = bracket_depth.saturating_add(1);
                buf.push(c);
            }
            ']' if !in_quotes && bracket_depth > 0 => {
                bracket_depth -= 1;
                buf.push(c);
            }
            ';' if !in_quotes && bracket_depth == 0 => {
                out.push(std::mem::take(&mut buf));
            }
            _ => buf.push(c),
        }
    }

    // An unterminated `"` here used to be fatal, but hand-typed
    // engineering formats like `# ?/8"` (the manifest.xlsx inch-mark
    // suffix) rely on POI's lenient tokenization — the trailing
    // unclosed `"` is treated as a literal char by the per-kind
    // formatter. Falling through lets the flushed buffer reach the
    // formatter, which handles the unterminated quote internally.
    // Bracket handling stays strict because `[…]` is structural
    // (conditions, colors, locales) and its failure modes aren't
    // recoverable without guessing the intended prefix semantics.
    if bracket_depth > 0 {
        return Err(FormatWarning::new(
            FormatWarningKind::MalformedFormatCode,
            code,
            String::new(),
            "unterminated bracket in format code",
        ));
    }

    // Flush the trailing buffer — including when it's empty, because
    // empty leading sections matter (e.g. `;;;@`).
    out.push(buf);
    Ok(out)
}

// ---------------------------------------------------------------------------
// Part parser — prefixes, body, kind, has_sign
// ---------------------------------------------------------------------------

fn parse_part(section: &str) -> Result<FormatSection, FormatWarning> {
    let (prefixes, body) = split_prefixes(section)?;

    let mut condition: Option<Condition> = None;
    let mut color: Option<String> = None;
    let mut locale: Option<String> = None;
    let mut currency_symbol: Option<String> = None;

    for prefix in prefixes {
        // `$...` inside brackets — currency / locale.
        if let Some(rest) = prefix.strip_prefix('$') {
            let (sym, loc) = match rest.find('-') {
                Some(dash) => (&rest[..dash], Some(&rest[dash + 1..])),
                None => (rest, None),
            };
            if !sym.is_empty() {
                currency_symbol = Some(sym.to_string());
            }
            if let Some(l) = loc
                && !l.is_empty()
            {
                locale = Some(l.to_string());
            }
            continue;
        }
        // `-XXX` inside brackets — bare locale id.
        if let Some(rest) = prefix.strip_prefix('-')
            && rest.chars().all(|c| c.is_ascii_hexdigit())
            && !rest.is_empty()
        {
            locale = Some(rest.to_string());
            continue;
        }
        // Condition. `[>=100]`, `[<-5]`, `[=0]`, `[<>0]`, etc.
        if let Some(parsed_cond) = try_parse_condition(&prefix) {
            condition = Some(parsed_cond);
            continue;
        }
        // Colour code — anything else that looks alphabetic.
        //
        // POI allows "Black", "Red", "Green", "White", "Blue",
        // "Magenta", "Yellow", "Cyan", and "Color N" where N is 1-56.
        // We don't enforce the whitelist; any remaining prefix is
        // preserved for downstream inspection.
        color = Some(prefix.to_ascii_lowercase());
    }

    let kind = classify_kind(&body);
    let has_sign = body_has_sign(&body);

    Ok(FormatSection {
        body,
        condition,
        color,
        locale,
        currency_symbol,
        kind,
        has_sign,
    })
}

/// Peel up to three `[...]` *prefixes* off the front of a section and
/// return them plus the remaining body. Quoted literals and escapes
/// inside the body are respected.
///
/// Not every leading bracket is a prefix: elapsed-time markers like
/// `[h]`, `[mm]`, `[ss]` look syntactically identical to a `[Red]`
/// colour code, but POI treats them as body content because they drive
/// the per-kind formatter rather than gating selection. We classify
/// each candidate bracket against [`is_prefix_bracket`]; the first
/// non-prefix bracket terminates the prefix scan and the body starts
/// at that point. Without this rule `[h]:mm` would land with colour
/// `"h"` and body `":mm"`, and the elapsed-time formatter would never
/// see the `[h]` it needs.
fn split_prefixes(section: &str) -> Result<(Vec<String>, String), FormatWarning> {
    let mut prefixes = Vec::new();
    let mut rest = section.trim_start_matches(' ');
    while let Some(stripped) = rest.strip_prefix('[') {
        // Find the matching `]` at this level. POI prefixes don't nest,
        // so the first `]` closes the bracket. A quoted `"` can live
        // inside a prefix (rare but legal: `[$"€"-40C]`). Walk char-by-
        // char to honour the quote rule.
        let mut in_quotes = false;
        let mut found = None;
        for (idx, ch) in stripped.char_indices() {
            match ch {
                '"' => in_quotes = !in_quotes,
                ']' if !in_quotes => {
                    found = Some((idx, idx + ch.len_utf8()));
                    break;
                }
                _ => {}
            }
        }
        let Some((inside_end, after_bracket)) = found else {
            return Err(FormatWarning::new(
                FormatWarningKind::MalformedFormatCode,
                section,
                String::new(),
                "unterminated bracket prefix in format section",
            ));
        };
        let inside = &stripped[..inside_end];
        if !is_prefix_bracket(inside) {
            // Body bracket (elapsed marker, unknown verb) — stop here
            // and leave the `[…]` for the body to consume.
            break;
        }
        prefixes.push(inside.to_string());
        rest = &stripped[after_bracket..];
        if prefixes.len() >= 3 {
            break;
        }
    }
    Ok((prefixes, rest.to_string()))
}

/// Decide whether a bracket's content is a prefix (selection /
/// presentation hint) versus body content (elapsed marker, etc.).
///
/// Prefix-kinds POI recognises:
///
/// * `$...` — currency / locale.
/// * `-XXX` — bare locale id (hex).
/// * `>=`, `<=`, `<>`, `>`, `<`, `=` followed by a number — condition.
/// * Alphabetic colour name (`Red`, `Black`, `Blue`, `Green`, `White`,
///   `Magenta`, `Yellow`, `Cyan`) or `Color N`.
///
/// Anything else — including the elapsed markers `h`, `m`, `s`, `hh`,
/// `mm`, `ss` — is body content.
fn is_prefix_bracket(inside: &str) -> bool {
    let trimmed = inside.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.starts_with('$') {
        return true;
    }
    if let Some(rest) = trimmed.strip_prefix('-')
        && !rest.is_empty()
        && rest.chars().all(|c| c.is_ascii_hexdigit())
    {
        return true;
    }
    if try_parse_condition(trimmed).is_some() {
        return true;
    }
    if is_known_color(trimmed) {
        return true;
    }
    false
}

fn is_known_color(token: &str) -> bool {
    let lower = token.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "black" | "red" | "green" | "white" | "blue" | "magenta" | "yellow" | "cyan"
    ) || lower.starts_with("color ")
}

/// Try to parse a bracket prefix like `>=100`, `<0`, `=0`, `<>10`.
/// Returns `None` if the prefix is not a condition.
fn try_parse_condition(prefix: &str) -> Option<Condition> {
    let p = prefix.trim();
    // Try two-char operators first.
    let (op_len, ctor): (usize, fn(f64) -> Condition) = if let Some(rest) = p.strip_prefix(">=") {
        (2 + (p.len() - 2 - rest.len()), Condition::GreaterOrEqual)
    } else if p.starts_with(">=") {
        (2, Condition::GreaterOrEqual)
    } else if p.starts_with("<=") {
        (2, Condition::LessOrEqual)
    } else if p.starts_with("<>") {
        (2, Condition::NotEqual)
    } else if p.starts_with('>') {
        (1, Condition::GreaterThan)
    } else if p.starts_with('<') {
        (1, Condition::LessThan)
    } else if p.starts_with('=') {
        (1, Condition::Equal)
    } else {
        return None;
    };
    let num_part = p[op_len..].trim();
    let value: f64 = num_part.parse().ok()?;
    Some(ctor(value))
}

/// Classify the body into a [`SectionKind`].
///
/// Detection order follows POI: Elapsed (has `[h]`/`[m]`/`[s]`) beats
/// Date (has calendrical tokens) beats Text (contains `@`) beats
/// General (keyword) beats Number (default).
fn classify_kind(body: &str) -> SectionKind {
    let lower = body.to_ascii_lowercase();

    // Elapsed markers live in bracket prefixes that survived
    // split_prefixes (POI puts them in the body, not the prefix list).
    // Scan for `[h]`, `[m]`, `[s]` as substrings.
    let mut in_quotes = false;
    let mut i = 0;
    let bytes = lower.as_bytes();
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'"' {
            in_quotes = !in_quotes;
        } else if !in_quotes && b == b'[' {
            // Check for `[h`, `[m`, `[s`.
            let rest = &bytes[i + 1..];
            if matches!(rest.first(), Some(b'h') | Some(b'm') | Some(b's'))
                && rest
                    .get(1..)
                    .map(|r| {
                        r.starts_with(b"]")
                            || r.starts_with(b"h]")
                            || r.starts_with(b"m]")
                            || r.starts_with(b"s]")
                            || r.starts_with(b"hh]")
                            || r.starts_with(b"mm]")
                            || r.starts_with(b"ss]")
                    })
                    .unwrap_or(false)
            {
                return SectionKind::Elapsed;
            }
        }
        i += 1;
    }

    // Text: `@` outside quotes.
    if has_unquoted(&lower, b'@') {
        return SectionKind::Text;
    }

    // General keyword.
    if is_general_body(&lower) {
        return SectionKind::General;
    }

    // Date: any of y/d/h/s, or `am/pm`, `a/p`. The `m` token is handled
    // in a second pass below because it's ambiguous — it could be month,
    // minute, or a literal character.
    let has_unambiguous_date_token = has_unquoted(&lower, b'y')
        || has_unquoted(&lower, b'd')
        || has_unquoted(&lower, b'h')
        || has_unquoted(&lower, b's')
        || lower.contains("am/pm")
        || lower.contains("a/p");
    if has_unambiguous_date_token {
        return SectionKind::Date;
    }

    // Lone `m` run — POI classifies bodies like `mmmm`, `mm`, `m` (no
    // companion `y/d/h/s`, no numeric placeholders) as Date so they
    // render as month names/numbers. The safeguard is "no numeric
    // placeholders in the body": `0m` is a Number with a literal `m`
    // suffix; `mmm` with no digits is a month token. This matches POI's
    // CellFormatPart kind detection.
    if has_unquoted(&lower, b'm')
        && !has_unquoted(&lower, b'0')
        && !has_unquoted(&lower, b'#')
        && !has_unquoted(&lower, b'?')
    {
        return SectionKind::Date;
    }

    SectionKind::Number
}

/// POI's `hasSign()` — any literal `-`, `(`, or `)` outside quotes /
/// brackets / escape sequences.
fn body_has_sign(body: &str) -> bool {
    let mut in_quotes = false;
    let mut bracket_depth = 0u32;
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => in_quotes = !in_quotes,
            '\\' if !in_quotes => {
                // Escape consumes the next char — and `\-` is the
                // canonical way to declare a literal minus, which DOES
                // count as a sign marker under POI's rule.
                if matches!(chars.peek(), Some('-') | Some('(') | Some(')')) {
                    return true;
                }
                let _ = chars.next();
            }
            '[' if !in_quotes => bracket_depth = bracket_depth.saturating_add(1),
            ']' if !in_quotes && bracket_depth > 0 => bracket_depth -= 1,
            '-' | '(' | ')' if !in_quotes && bracket_depth == 0 => return true,
            _ => {}
        }
    }
    false
}

fn has_unquoted(body: &str, target: u8) -> bool {
    let mut in_quotes = false;
    let mut escape = false;
    for &b in body.as_bytes() {
        if escape {
            escape = false;
            continue;
        }
        match b {
            b'\\' if !in_quotes => escape = true,
            b'"' => in_quotes = !in_quotes,
            _ if in_quotes => {}
            _ if b == target => return true,
            _ => {}
        }
    }
    false
}

fn is_general_body(body: &str) -> bool {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return true;
    }
    trimmed.eq_ignore_ascii_case("general")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(code: &str) -> ParsedFormat {
        split_format(code).expect("format code should parse")
    }

    // --- Section split ------------------------------------------------------

    #[test]
    fn single_section_is_preserved_verbatim() {
        let p = parse("0.00");
        assert_eq!(p.sections.len(), 1);
        assert_eq!(p.sections[0].body, "0.00");
    }

    #[test]
    fn two_sections_split_on_semicolon() {
        let p = parse("0.00;(0.00)");
        assert_eq!(p.sections.len(), 2);
        assert_eq!(p.sections[0].body, "0.00");
        assert_eq!(p.sections[1].body, "(0.00)");
    }

    #[test]
    fn four_sections_split_in_order() {
        let p = parse("#,##0.00;-#,##0.00;\"zero\";@");
        assert_eq!(p.sections.len(), 4);
        assert_eq!(p.sections[0].body, "#,##0.00");
        assert_eq!(p.sections[1].body, "-#,##0.00");
        assert_eq!(p.sections[2].body, "\"zero\"");
        assert_eq!(p.sections[3].body, "@");
    }

    #[test]
    fn semicolon_inside_quotes_does_not_split() {
        let p = parse(r#""a;b";0"#);
        assert_eq!(p.sections.len(), 2);
        assert_eq!(p.sections[0].body, r#""a;b""#);
        assert_eq!(p.sections[1].body, "0");
    }

    #[test]
    fn semicolon_inside_brackets_does_not_split() {
        let p = parse("[>=100]0.0;[<0]0.0");
        assert_eq!(p.sections.len(), 2);
        assert_eq!(p.sections[0].body, "0.0");
        assert_eq!(p.sections[1].body, "0.0");
    }

    #[test]
    fn backslash_escape_protects_semicolon() {
        let p = parse(r"0\;0;0.0");
        assert_eq!(p.sections.len(), 2);
        assert_eq!(p.sections[0].body, r"0\;0");
        assert_eq!(p.sections[1].body, "0.0");
    }

    #[test]
    fn unterminated_quote_falls_through_leniently() {
        // POI-aligned: the section splitter no longer rejects an
        // unterminated `"` (e.g. `# ?/8"` for the manifest.xlsx
        // inch-mark). The remainder reaches the per-kind formatter,
        // which handles the trailing quote as a literal.
        let parsed = split_format("0.00\"unclosed").expect("lenient split");
        assert_eq!(parsed.sections.len(), 1);
        assert_eq!(parsed.sections[0].body, "0.00\"unclosed");
    }

    #[test]
    fn unterminated_bracket_returns_malformed() {
        let err = split_format("[>=100 0.0").unwrap_err();
        assert_eq!(err.kind, FormatWarningKind::MalformedFormatCode);
    }

    // --- Prefix strip -------------------------------------------------------

    #[test]
    fn locale_prefix_is_extracted() {
        let p = parse("[$-409]#,##0.00");
        assert_eq!(p.sections[0].locale.as_deref(), Some("409"));
        assert_eq!(p.sections[0].body, "#,##0.00");
    }

    #[test]
    fn currency_plus_locale_prefix_is_extracted() {
        let p = parse("[$$-409]#,##0.00");
        assert_eq!(p.sections[0].currency_symbol.as_deref(), Some("$"));
        assert_eq!(p.sections[0].locale.as_deref(), Some("409"));
        assert_eq!(p.sections[0].body, "#,##0.00");
    }

    #[test]
    fn color_prefix_is_extracted() {
        let p = parse("[Red]#,##0.00");
        assert_eq!(p.sections[0].color.as_deref(), Some("red"));
        assert_eq!(p.sections[0].body, "#,##0.00");
    }

    #[test]
    fn condition_prefix_is_extracted() {
        let p = parse("[>=100]0.00");
        assert_eq!(
            p.sections[0].condition,
            Some(Condition::GreaterOrEqual(100.0))
        );
        assert_eq!(p.sections[0].body, "0.00");
    }

    #[test]
    fn multiple_prefixes_all_extracted() {
        let p = parse("[Red][>=100][$$-409]#,##0");
        let s = &p.sections[0];
        assert_eq!(s.color.as_deref(), Some("red"));
        assert_eq!(s.condition, Some(Condition::GreaterOrEqual(100.0)));
        assert_eq!(s.currency_symbol.as_deref(), Some("$"));
        assert_eq!(s.locale.as_deref(), Some("409"));
        assert_eq!(s.body, "#,##0");
    }

    // --- Kind detection -----------------------------------------------------

    #[test]
    fn number_body_classifies_as_number() {
        assert_eq!(parse("#,##0.00").sections[0].kind, SectionKind::Number);
        assert_eq!(parse("0.0%").sections[0].kind, SectionKind::Number);
        // Scientific is still a Number body — exponent is a Number-formatter feature.
        assert_eq!(parse("0.00E+00").sections[0].kind, SectionKind::Number);
    }

    #[test]
    fn date_body_classifies_as_date() {
        assert_eq!(parse("yyyy-mm-dd").sections[0].kind, SectionKind::Date);
        assert_eq!(parse("h:mm AM/PM").sections[0].kind, SectionKind::Date);
        assert_eq!(
            parse("dddd, mmmm d, yyyy").sections[0].kind,
            SectionKind::Date
        );
    }

    #[test]
    fn elapsed_body_classifies_as_elapsed() {
        assert_eq!(parse("[h]:mm").sections[0].kind, SectionKind::Elapsed);
        assert_eq!(parse("[mm]:ss").sections[0].kind, SectionKind::Elapsed);
        assert_eq!(parse("[ss].000").sections[0].kind, SectionKind::Elapsed);
    }

    #[test]
    fn text_body_classifies_as_text() {
        assert_eq!(parse("@").sections[0].kind, SectionKind::Text);
    }

    #[test]
    fn general_body_classifies_as_general() {
        assert_eq!(parse("General").sections[0].kind, SectionKind::General);
        assert_eq!(parse("general").sections[0].kind, SectionKind::General);
    }

    // --- has_sign — the hasSign() contract -----------------------------------

    #[test]
    fn positive_body_has_no_sign() {
        assert!(!parse("#,##0.00").sections[0].has_sign);
        assert!(!parse("0.0%").sections[0].has_sign);
    }

    #[test]
    fn negative_with_literal_minus_has_sign() {
        let p = parse("0.00;-0.00");
        assert!(!p.sections[0].has_sign);
        assert!(
            p.sections[1].has_sign,
            "section 2 literal `-` must set has_sign"
        );
    }

    #[test]
    fn accounting_parens_has_sign() {
        let p = parse("#,##0.00;(#,##0.00)");
        assert!(!p.sections[0].has_sign);
        assert!(p.sections[1].has_sign, "section 2 parens must set has_sign");
    }

    #[test]
    fn quoted_dash_does_not_count_as_sign() {
        // `"--"` inside quotes is a literal run, not a sign marker.
        let p = parse(r#""--"0.00;"--"0.00"#);
        assert!(!p.sections[0].has_sign);
        assert!(!p.sections[1].has_sign);
    }

    #[test]
    fn escaped_dash_counts_as_sign() {
        // `\-` is the canonical declaration of a literal minus; POI
        // treats it as a sign marker so the engine doesn't double up.
        let p = parse(r"0.00;\-0.00");
        assert!(p.sections[1].has_sign);
    }

    #[test]
    fn watchlist_custom_bps_section_has_sign() {
        // The exact format that produces `-+1bps` on calamine. POI's
        // has_sign on the negative section is what the new pipeline
        // uses to stop prepending its own minus.
        let p = parse(r#"+0"bps";-0"bps""#);
        assert_eq!(p.sections.len(), 2);
        assert!(!p.sections[0].has_sign); // `+` does NOT count — only `-`, `(`, `)`
        assert!(
            p.sections[1].has_sign,
            "negative section literal `-` must set has_sign"
        );
    }

    // --- selectFormat dispatch ---------------------------------------------

    #[test]
    fn one_section_selects_always_zero() {
        let p = parse("0.00");
        assert_eq!(select_section_index(&p, 42.0), 0);
        assert_eq!(select_section_index(&p, -42.0), 0);
        assert_eq!(select_section_index(&p, 0.0), 0);
    }

    #[test]
    fn two_sections_split_pos_zero_vs_neg() {
        let p = parse("0.00;-0.00");
        assert_eq!(select_section_index(&p, 42.0), 0);
        assert_eq!(select_section_index(&p, 0.0), 0);
        assert_eq!(select_section_index(&p, -42.0), 1);
    }

    #[test]
    fn three_sections_split_pos_neg_zero() {
        let p = parse("0.00;-0.00;\"zero\"");
        assert_eq!(select_section_index(&p, 42.0), 0);
        assert_eq!(select_section_index(&p, -42.0), 1);
        assert_eq!(select_section_index(&p, 0.0), 2);
    }

    #[test]
    fn four_sections_split_pos_neg_zero_text() {
        let p = parse("0.00;-0.00;\"zero\";@");
        assert_eq!(select_section_index(&p, 42.0), 0);
        assert_eq!(select_section_index(&p, -42.0), 1);
        assert_eq!(select_section_index(&p, 0.0), 2);
        // Text section index is 3; numeric caller never lands on it.
    }

    #[test]
    fn conditional_sections_select_by_match() {
        let p = parse("[>=100]0.0;[<0]0.0;0.0");
        assert_eq!(select_section_index(&p, 150.0), 0);
        assert_eq!(select_section_index(&p, -5.0), 1);
        assert_eq!(select_section_index(&p, 50.0), 2);
    }
}
