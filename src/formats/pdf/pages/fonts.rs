/// Detect bold from font name (common patterns: "Bold", "Heavy", "Black", "-B").
pub(crate) fn is_bold_font(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("bold")
        || lower.contains("heavy")
        || lower.contains("black")
        || lower.ends_with("-b")
        || lower.ends_with(",bold")
}

/// Detect non-bold from font name — overrides font_weight when the name
/// explicitly indicates a regular/light weight. This catches cases where
/// pdf_oxide reports Bold for fonts like "Myriad-Roman" where the PDF's
/// font descriptor has an incorrect weight.
pub(crate) fn is_non_bold_font(name: &str) -> bool {
    let lower = name.to_lowercase();
    // Match "roman" as a word in any position: "-Roman", " Roman", "Roman"
    // Same for other non-bold weight indicators.
    lower.contains("roman")
        || lower.contains("regular")
        || lower.contains("light")
        || lower.contains("book")
        || lower.contains("thin")
        || lower.contains("medium")
}

/// Detect italic from font name (common patterns: "Italic", "Oblique", "-I").
pub(crate) fn is_italic_font(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("italic")
        || lower.contains("oblique")
        || lower.ends_with("-i")
        || lower.ends_with(",italic")
}

/// Clean up font name: strip subset prefixes (e.g., "ABCDEF+Arial" -> "Arial").
pub(crate) fn clean_font_name(name: &str) -> String {
    // PDF subset fonts have a 6-char uppercase prefix followed by '+'.
    if name.len() > 7 && name.as_bytes()[6] == b'+' {
        let prefix = &name[..6];
        if prefix.chars().all(|c| c.is_ascii_uppercase()) {
            return name[7..].to_string();
        }
    }
    name.to_string()
}
