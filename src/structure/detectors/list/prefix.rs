/// Information about a detected list prefix.
pub(super) struct ListPrefixInfo {
    /// Whether this is an ordered list item (numbered/lettered).
    pub(super) ordered: bool,
}

/// Detect whether a text string starts with a list bullet or number prefix.
///
/// Returns `Some(ListPrefixInfo)` if a prefix is found, `None` otherwise.
pub(super) fn detect_list_prefix(text: &str) -> Option<ListPrefixInfo> {
    let trimmed = text.trim_start();

    // Unordered bullet characters.
    let unordered_bullets = [
        '•', '–', '—', '◦', '▪', '‣', '○', '●', '■', '□', '▸', '▹', '►',
    ];

    // Check for unordered bullet at start.
    if let Some(first_char) = trimmed.chars().next()
        && unordered_bullets.contains(&first_char)
    {
        // Must be followed by a space or be the only char.
        let rest = &trimmed[first_char.len_utf8()..];
        if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') {
            return Some(ListPrefixInfo { ordered: false });
        }
    }

    // Check for "- " or "* " (ASCII bullet markers).
    if (trimmed.starts_with("- ") || trimmed.starts_with("* ")) && trimmed.len() > 2 {
        return Some(ListPrefixInfo { ordered: false });
    }

    // Check for ordered prefixes: "1.", "1)", "(1)", "a.", "a)", "(a)",
    // "i.", "i)", "(i)", "A.", "A)", etc.
    if detect_ordered_prefix(trimmed) {
        return Some(ListPrefixInfo { ordered: true });
    }

    None
}

/// Detect ordered list prefixes: numbers, letters, or roman numerals
/// followed by `.` or `)`, optionally wrapped in parentheses.
fn detect_ordered_prefix(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.is_empty() {
        return false;
    }

    // Pattern: "(X)" where X is a number, letter, or roman numeral.
    if bytes[0] == b'(' {
        if let Some(close) = text.find(')') {
            let inner = &text[1..close];
            if is_ordered_label(inner) {
                // Must be followed by a space.
                let rest = &text[close + 1..];
                return rest.is_empty() || rest.starts_with(' ');
            }
        }
        return false;
    }

    // Pattern: "X." or "X)" where X is a number, letter, or roman numeral.
    // Find the delimiter (. or ))
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'.' || b == b')' {
            let label = &text[..i];
            if is_ordered_label(label) {
                // Must be followed by a space or end of string.
                let rest = &text[i + 1..];
                return rest.is_empty() || rest.starts_with(' ');
            }
            return false;
        }
        // Allow digits, letters, and roman numeral chars.
        if !b.is_ascii_alphanumeric() {
            return false;
        }
        // Don't look too far — labels longer than 6 chars aren't list markers.
        if i >= 6 {
            return false;
        }
    }

    false
}

/// Check if a string is a valid ordered list label.
///
/// Valid labels: single digits (1-99), single letters (a-z, A-Z),
/// or roman numerals (i, ii, iii, iv, v, vi, vii, viii, ix, x, etc.).
fn is_ordered_label(s: &str) -> bool {
    if s.is_empty() || s.len() > 6 {
        return false;
    }

    // Pure digits (1-99).
    if let Ok(n) = s.parse::<u32>() {
        return (1..=99).contains(&n);
    }

    // Single letter (a-z, A-Z).
    if s.len() == 1 {
        let c = s.as_bytes()[0];
        return c.is_ascii_alphabetic();
    }

    // Roman numerals (lowercase or uppercase).
    is_roman_numeral(s)
}

/// Check if a string is a valid roman numeral (i–xii, I–XII, etc.).
fn is_roman_numeral(s: &str) -> bool {
    let lower = s.to_lowercase();
    matches!(
        lower.as_str(),
        "i" | "ii"
            | "iii"
            | "iv"
            | "v"
            | "vi"
            | "vii"
            | "viii"
            | "ix"
            | "x"
            | "xi"
            | "xii"
            | "xiii"
            | "xiv"
            | "xv"
            | "xvi"
            | "xvii"
            | "xviii"
            | "xix"
            | "xx"
    )
}

/// Detect definition list pattern: "term: definition".
///
/// Pattern matching:
/// - `term` is 1-4 words (no colons)
/// - followed by `: ` (colon + space)
/// - followed by at least 1 word of definition text
///
/// Returns `Some(colon_position)` if matched, `None` otherwise.
pub(super) fn detect_definition_prefix(text: &str) -> Option<usize> {
    let trimmed = text.trim();

    // Find the first colon
    let colon_pos = trimmed.find(':')?;

    // Extract term (before colon) and definition (after colon + space)
    let term = &trimmed[..colon_pos];
    let rest = &trimmed[colon_pos + 1..];

    // Must have ": " (colon followed by space)
    if !rest.starts_with(' ') {
        return None;
    }

    let term_trimmed = term.trim();
    let definition_trimmed = rest[1..].trim(); // skip the space

    // Definition must have at least 1 word
    if definition_trimmed.is_empty() {
        return None;
    }

    // Term must have 1-4 words (no colons in the term)
    if term_trimmed.is_empty() || term_trimmed.contains(':') {
        return None;
    }

    let term_word_count = term_trimmed.split_whitespace().count();
    if !(1..=4).contains(&term_word_count) {
        return None;
    }

    Some(colon_pos)
}
