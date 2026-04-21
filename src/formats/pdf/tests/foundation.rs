use super::*;

#[test]
fn decoder_name_and_extensions() {
    let dec = PdfDecoder;
    assert_eq!(dec.name(), "PDF");
    assert_eq!(dec.supported_extensions(), &["pdf"]);
}

#[test]
fn decode_invalid_data_returns_error() {
    let dec = PdfDecoder;
    let result = dec.decode(b"this is not a PDF".to_vec());
    assert!(result.is_err());
}

#[test]
fn metadata_invalid_data_returns_error() {
    let dec = PdfDecoder;
    let result = dec.metadata(b"not a pdf file");
    assert!(result.is_err());
}

#[test]
fn image_format_from_image_data() {
    let jpeg_data = ImageData::Jpeg(vec![0xFF, 0xD8, 0xFF]);
    let (_, fmt) = match &jpeg_data {
        ImageData::Jpeg(b) => (b.clone(), crate::model::ImageFormat::Jpeg),
        ImageData::Raw { pixels, .. } => (pixels.clone(), crate::model::ImageFormat::Unknown),
    };
    assert_eq!(fmt, crate::model::ImageFormat::Jpeg);
}

#[test]
fn extract_declared_document_language_prefers_catalog_lang() {
    let data = build_pdf_with_catalog_lang("fr-CA");
    let mut doc = PdfDocument::from_bytes(data).unwrap();
    let mut warnings = Vec::new();

    let lang = extract_declared_document_language(&mut doc, &mut warnings);
    assert_eq!(lang, Some("fr-CA".to_string()));
    assert!(warnings.is_empty());
}

#[test]
fn harmonize_text_direction_only_applies_without_span_signal() {
    assert_eq!(
        harmonize_text_direction_with_language(
            TextDirection::LeftToRight,
            ValueOrigin::Estimated,
            Some("he")
        ),
        (TextDirection::RightToLeft, ValueOrigin::Reconstructed)
    );

    assert_eq!(
        harmonize_text_direction_with_language(
            TextDirection::LeftToRight,
            ValueOrigin::Reconstructed,
            Some("he")
        ),
        (TextDirection::LeftToRight, ValueOrigin::Reconstructed)
    );
}
