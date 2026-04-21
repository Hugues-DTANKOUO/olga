use pdf_oxide::PdfDocument;
use pdf_oxide::extractors::xmp::XmpExtractor;
use pdf_oxide::object::Object;

use crate::error::{Warning, WarningKind};
use crate::model::{TextDirection, ValueOrigin};

pub(super) fn extract_declared_document_language(
    doc: &mut PdfDocument,
    warnings: &mut Vec<Warning>,
) -> Option<String> {
    let catalog_lang = extract_catalog_language(doc, warnings);
    let xmp_lang = match XmpExtractor::extract(doc) {
        Ok(Some(xmp)) => xmp.dc_language.and_then(normalize_language_tag),
        Ok(None) => None,
        Err(err) => {
            warnings.push(Warning {
                kind: WarningKind::PartialExtraction,
                message: format!("Failed to extract XMP language metadata: {}", err),
                page: None,
            });
            None
        }
    };

    if let (Some(catalog_lang), Some(xmp_lang)) = (&catalog_lang, &xmp_lang)
        && catalog_lang != xmp_lang
    {
        warnings.push(Warning {
            kind: WarningKind::PartialExtraction,
            message: format!(
                "Conflicting PDF language declarations: catalog /Lang='{}' vs XMP dc:language='{}'; using catalog /Lang",
                catalog_lang, xmp_lang
            ),
            page: None,
        });
    }

    catalog_lang.or(xmp_lang)
}

pub(super) fn harmonize_text_direction_with_language(
    text_direction: TextDirection,
    origin: ValueOrigin,
    document_lang: Option<&str>,
) -> (TextDirection, ValueOrigin) {
    if text_direction != TextDirection::LeftToRight || origin != ValueOrigin::Estimated {
        return (text_direction, origin);
    }

    match document_lang {
        Some(lang) if is_rtl_language_tag(lang) => {
            (TextDirection::RightToLeft, ValueOrigin::Reconstructed)
        }
        _ => (text_direction, origin),
    }
}

fn extract_catalog_language(doc: &mut PdfDocument, warnings: &mut Vec<Warning>) -> Option<String> {
    let catalog = match doc.catalog() {
        Ok(catalog) => catalog,
        Err(err) => {
            warnings.push(Warning {
                kind: WarningKind::PartialExtraction,
                message: format!("Failed to read PDF catalog while extracting /Lang: {}", err),
                page: None,
            });
            return None;
        }
    };

    let catalog_dict = catalog.as_dict()?;
    let lang = catalog_dict.get("Lang")?;

    match lang {
        Object::String(bytes) => {
            normalize_language_tag(String::from_utf8_lossy(bytes).into_owned())
        }
        _ => {
            warnings.push(Warning {
                kind: WarningKind::MalformedContent,
                message: "PDF catalog /Lang entry is not a string".to_string(),
                page: None,
            });
            None
        }
    }
}

fn normalize_language_tag(lang: impl Into<String>) -> Option<String> {
    let lang = lang.into();
    let trimmed = lang.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn is_rtl_language_tag(lang: &str) -> bool {
    let primary = lang
        .split(['-', '_'])
        .next()
        .unwrap_or(lang)
        .to_ascii_lowercase();
    matches!(
        primary.as_str(),
        "ar" | "fa" | "he" | "iw" | "ps" | "sd" | "ug" | "ur" | "yi"
    )
}
