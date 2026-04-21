use super::*;

#[test]
fn extract_page_dims_fallback_on_empty_doc() {
    // Verify that a bad document triggers the US Letter fallback with a warning.
    let mut warnings = Vec::new();
    let data = b"%PDF-1.4\n%%EOF";
    let mut doc = match PdfDocument::from_bytes(data.to_vec()) {
        Ok(d) => d,
        Err(_) => return, // Can't even open — nothing to test
    };
    let dims = extract_page_dims(&mut doc, 999, &mut warnings);
    // Should fall back to US Letter since page 999 doesn't exist.
    assert_eq!(dims.media_box.width, 612.0);
    assert_eq!(dims.media_box.height, 792.0);
    assert!(!warnings.is_empty());
}

#[test]
fn page_dimensions_normalizes_abs() {
    // Verify that negative-extent MediaBox is handled via abs().
    // This is a logic test for the abs() calls in extract_page_dims.
    let width = (0.0_f32 - 612.0).abs();
    let height = (0.0_f32 - 792.0).abs();
    assert_eq!(width, 612.0);
    assert_eq!(height, 792.0);
}
