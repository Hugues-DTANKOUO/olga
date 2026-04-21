//! POI-aligned CellTextFormatter — renders a Text-kind section body
//! against a raw cell-text value.
//!
//! # What "text section" means
//!
//! A 4-section format `pos;neg;zero;text` reserves the fourth section
//! for *string* cell values. Numeric cells never land on this section;
//! they dispatch through the first three based on sign. Only when a
//! user types literal text into a cell (or, once #46 lands the
//! emit-pass swap, when we feed a boolean / formula-error through the
//! same channel) does the text body get invoked.
//!
//! # Grammar
//!
//! * `@` — substitute the original cell text verbatim.
//! * `"..."` — quoted literal; contents rendered as-is.
//! * `\x` — escape the following character.
//! * `_x` — padding directive; emits one space (no column-width
//!   information at extraction time).
//! * `*x` — fill directive; dropped.
//! * any other character — literal (POI's text body accepts
//!   essentially anything outside the six special forms above).
//!
//! # Relationship to the degradation contract
//!
//! The only failure mode is a malformed body (unterminated quote,
//! trailing backslash). Everything else renders without warnings —
//! text sections are defined permissively.

use super::contract::{FormatWarning, FormatWarningKind};

/// Render a Text-kind section body against the raw cell text.
///
/// * `text` — the original string payload of the cell. Substituted at
///   every `@` occurrence in `body`.
/// * `body` — body text of the chosen `FormatSection`, already
///   stripped of bracket prefixes by the section splitter (#39).
///
/// Returns `Err` only when the body is syntactically broken.
pub(super) fn format_text_body(text: &str, body: &str) -> Result<String, FormatWarning> {
    let chars: Vec<char> = body.chars().collect();
    let n = chars.len();
    let mut i = 0usize;
    let mut out = String::new();

    while i < n {
        let c = chars[i];
        match c {
            '@' => {
                out.push_str(text);
                i += 1;
            }
            '"' => {
                let start = i + 1;
                let mut j = start;
                while j < n && chars[j] != '"' {
                    j += 1;
                }
                if j >= n {
                    return Err(FormatWarning::new(
                        FormatWarningKind::MalformedFormatCode,
                        body,
                        text,
                        "unterminated quoted literal in text body",
                    ));
                }
                out.extend(chars[start..j].iter());
                i = j + 1;
            }
            '\\' => {
                if i + 1 >= n {
                    return Err(FormatWarning::new(
                        FormatWarningKind::MalformedFormatCode,
                        body,
                        text,
                        "trailing backslash in text body",
                    ));
                }
                out.push(chars[i + 1]);
                i += 2;
            }
            '_' => {
                out.push(' ');
                // Consume the width-reference char (if any).
                i = (i + 2).min(n);
            }
            '*' => {
                // Fill directive — drop; consume next char.
                i = (i + 2).min(n);
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_at_echoes_text() {
        assert_eq!(format_text_body("hello", "@").unwrap(), "hello");
    }

    #[test]
    fn quoted_prefix_plus_at() {
        assert_eq!(
            format_text_body("alice", "\"Name: \"@").unwrap(),
            "Name: alice"
        );
    }

    #[test]
    fn at_plus_quoted_suffix() {
        assert_eq!(
            format_text_body("alice", "@\" (customer)\"").unwrap(),
            "alice (customer)"
        );
    }

    #[test]
    fn at_twice_duplicates_text() {
        assert_eq!(format_text_body("x", "@/@").unwrap(), "x/x");
    }

    #[test]
    fn backslash_escape_renders_next_char() {
        assert_eq!(format_text_body("x", r"\@@").unwrap(), "@x");
    }

    #[test]
    fn underscore_pad_emits_single_space() {
        assert_eq!(format_text_body("x", "@_)").unwrap(), "x ");
    }

    #[test]
    fn star_fill_is_dropped() {
        assert_eq!(format_text_body("x", "@*-").unwrap(), "x");
    }

    #[test]
    fn body_without_at_is_just_literals() {
        assert_eq!(
            format_text_body("ignored", "\"hello world\"").unwrap(),
            "hello world"
        );
    }

    #[test]
    fn empty_body_renders_empty() {
        assert_eq!(format_text_body("anything", "").unwrap(), "");
    }

    #[test]
    fn unicode_text_passes_through() {
        assert_eq!(format_text_body("café", "@").unwrap(), "café");
    }

    #[test]
    fn unterminated_quote_returns_malformed() {
        let err = format_text_body("x", "@\"unclosed").unwrap_err();
        assert_eq!(err.kind, FormatWarningKind::MalformedFormatCode);
    }

    #[test]
    fn trailing_backslash_returns_malformed() {
        let err = format_text_body("x", "@\\").unwrap_err();
        assert_eq!(err.kind, FormatWarningKind::MalformedFormatCode);
    }
}
