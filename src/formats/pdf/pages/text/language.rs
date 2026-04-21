//! Document-level language inference from page-zero character content.
//!
//! Used by the PDF decoder when the PDF catalog does not declare a `/Lang`
//! entry. We inspect the first page's characters and infer the language tag
//! from CJK script density or from Arabic/Hebrew RTL runs.

use pdf_oxide::text::rtl_detector::is_rtl_text;
use pdf_oxide::text::script_detector::{
    detect_cjk_script, infer_document_language as infer_cjk_document_language,
};
use pdf_oxide::text::{CJKScript, DocumentLanguage as CjkDocumentLanguage};

pub(crate) fn infer_document_language_tag(chars: &[char]) -> Option<String> {
    let mut han = 0;
    let mut hiragana = 0;
    let mut katakana = 0;
    let mut hangul = 0;
    let mut arabic = 0;
    let mut hebrew = 0;

    for ch in chars {
        match detect_cjk_script(*ch as u32) {
            Some(CJKScript::Han)
            | Some(CJKScript::HanExtensionA)
            | Some(CJKScript::HanExtensionBF) => {
                han += 1;
            }
            Some(CJKScript::Hiragana) => hiragana += 1,
            Some(CJKScript::Katakana) | Some(CJKScript::HalfwidthKatakana) => katakana += 1,
            Some(CJKScript::Hangul) => hangul += 1,
            Some(CJKScript::CJKSymbol) | None => {}
        }

        match is_rtl_text(*ch as u32) {
            true if matches!(*ch as u32, 0x0590..=0x05FF) => hebrew += 1,
            true => arabic += 1,
            false => {}
        }
    }

    let cjk_scripts = [
        (CJKScript::Han, han),
        (CJKScript::Hiragana, hiragana),
        (CJKScript::Katakana, katakana),
        (CJKScript::Hangul, hangul),
    ]
    .into_iter()
    .filter(|(_, count)| *count > 0)
    .collect::<Vec<_>>();

    if let Some(language) = infer_cjk_document_language(&cjk_scripts) {
        return Some(
            match language {
                CjkDocumentLanguage::Japanese => "ja",
                CjkDocumentLanguage::Korean => "ko",
                CjkDocumentLanguage::Chinese => "zh",
            }
            .to_string(),
        );
    }

    if arabic > hebrew && arabic > 0 {
        return Some("ar".to_string());
    }
    if hebrew > 0 {
        return Some("he".to_string());
    }

    None
}
