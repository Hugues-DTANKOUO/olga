//! Shared XML helpers for format decoders.
//!
//! These utilities handle namespace-agnostic tag matching, attribute extraction,
//! and common OOXML patterns. Reusable across DOCX, HTML, and future XML-based decoders.

use crate::model::{Color, ImageFormat};

/// Extract the local name from a potentially namespaced tag (e.g., `b"w:p"` → `b"p"`).
/// Returns an owned `Vec<u8>` to avoid lifetime issues with quick-xml temporaries.
pub fn local_name(full: &[u8]) -> Vec<u8> {
    full.iter()
        .position(|&b| b == b':')
        .map(|i| full[i + 1..].to_vec())
        .unwrap_or_else(|| full.to_vec())
}

/// Check if a tag name is in the `w:` (wordprocessingml) namespace.
/// Tags without a namespace prefix are also treated as `w:`.
pub fn is_w_element(full: &[u8]) -> bool {
    full.starts_with(b"w:") || !full.contains(&b':')
}

/// Check if a tag name is in the `mc:` (markup compatibility) namespace.
pub fn is_mc_element(full: &[u8]) -> bool {
    full.starts_with(b"mc:")
}

/// Extract attribute value as a String with XML entities decoded.
///
/// In XML the five predefined entities (`&amp;`, `&lt;`, `&gt;`, `&apos;`,
/// `&quot;`) and numeric character references (`&#x2014;` etc.) may appear
/// inside attribute values. The raw bytes in `attr.value` preserve them
/// verbatim, so passing those bytes through `String::from_utf8_lossy` would
/// surface "Notes &amp; Assumptions" instead of "Notes & Assumptions" — a
/// double-encoding that every mainstream XML consumer (python xml.etree,
/// java StAX, ...) avoids by running an unescape pass on attribute values.
///
/// We delegate to `Attribute::unescape_value`, which handles the predefined
/// entities and numeric references. If the raw bytes are not well-formed
/// (e.g. an unterminated reference), we fall back to the lossy UTF-8 form
/// rather than erroring — attribute access is on the hot path and a
/// malformed attribute is not worth aborting the entire decode for.
pub fn attr_value(attr: &quick_xml::events::attributes::Attribute) -> String {
    let raw = String::from_utf8_lossy(&attr.value);
    unescape_xml_entities(raw.as_ref())
}

/// Decode the five XML predefined entities plus numeric character
/// references (`&#x2014;`, `&#8212;`). Unknown named entities are passed
/// through unchanged.
fn unescape_xml_entities(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'&'
            && let Some(end_rel) = input[i + 1..].find(';')
        {
            let end = i + 1 + end_rel;
            let name = &input[i + 1..end];
            if let Some(decoded) = resolve_predefined_entity(name) {
                out.push(decoded);
                i = end + 1;
                continue;
            } else if let Some(stripped) = name.strip_prefix('#') {
                let (radix, digits) = if let Some(hex) = stripped.strip_prefix(['x', 'X']) {
                    (16, hex)
                } else {
                    (10, stripped)
                };
                if let Ok(code) = u32::from_str_radix(digits, radix)
                    && let Some(ch) = char::from_u32(code)
                {
                    out.push(ch);
                    i = end + 1;
                    continue;
                }
            }
        }
        // Fallback: copy a single char.
        let ch = input[i..].chars().next().expect("non-empty slice");
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Parse a boolean attribute. In OOXML, `<w:b/>` means true.
/// `<w:b w:val="false"/>` or `<w:b w:val="0"/>` means false.
pub fn bool_attr(e: &quick_xml::events::BytesStart, default: bool) -> bool {
    for attr in e.attributes().flatten() {
        if local_name(attr.key.as_ref()) == b"val" {
            let v = attr_value(&attr);
            return !matches!(v.as_str(), "false" | "0" | "off" | "no");
        }
    }
    default // No val attribute = default (usually true for empty element)
}

/// Parse hex RGB color string (e.g., "FF0000") → Color.
pub fn parse_hex_color(hex: &str) -> Option<Color> {
    let hex = hex.trim_start_matches('#');
    if hex.len() < 6 || hex == "auto" {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::rgb(r, g, b))
}

/// Resolve a general entity reference name to its character value.
///
/// In quick-xml 0.38+/0.39+, predefined XML entities (`&amp;`, `&lt;`, `&gt;`,
/// `&apos;`, `&quot;`) are reported as `Event::GeneralRef` instead of being
/// inlined into `Event::Text`.  This helper maps the entity name (without
/// the `&` and `;` delimiters) to the corresponding character.
pub fn resolve_predefined_entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "apos" => Some('\''),
        "quot" => Some('"'),
        _ => None,
    }
}

/// Guess image format from magic bytes.
pub fn guess_image_format(data: &[u8]) -> ImageFormat {
    if data.starts_with(b"\x89PNG") {
        ImageFormat::Png
    } else if data.starts_with(b"\xFF\xD8\xFF") {
        ImageFormat::Jpeg
    } else if data.starts_with(b"GIF8") {
        ImageFormat::Gif
    } else if data.starts_with(b"BM") {
        ImageFormat::Bmp
    } else if data.starts_with(b"II") || data.starts_with(b"MM") {
        ImageFormat::Tiff
    } else if data.starts_with(b"<svg") || data.starts_with(b"<?xml") {
        ImageFormat::Svg
    } else if data.starts_with(b"RIFF") && data.len() > 12 && &data[8..12] == b"WEBP" {
        ImageFormat::Webp
    } else {
        ImageFormat::Unknown
    }
}
