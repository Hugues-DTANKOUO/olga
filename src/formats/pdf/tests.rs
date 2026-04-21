use super::artifact_image::convert_images;
use super::artifact_risk::{
    RepeatedImageArtifactCandidate, RepeatedImageArtifactSignature,
    RepeatedVectorArtifactCandidate, RepeatedVectorArtifactSignature,
    emit_repeated_image_artifact_risk_warnings, emit_repeated_vector_artifact_risk_warnings,
    repeated_image_artifact_signature, repeated_vector_artifact_signature,
};
use super::artifact_text::{
    ArtifactBand, ArtifactRuleSource, HeuristicArtifactCandidate, TextArtifactRule,
    TextArtifactSuppressionPlan, add_repeated_tagged_residual_rules, suppress_artifact_text_spans,
};
use super::language::{extract_declared_document_language, harmonize_text_direction_with_language};
use super::marked_content::{
    MarkedContentContext, active_marked_content_artifact, active_marked_content_mcid,
    extract_artifact_type_from_properties_dict,
};
use super::paths::classify::build_artifact_path_candidate;
use super::paths::scan::finalize_artifact_path_candidate;
use super::paths::{
    ArtifactPathBuilder, ArtifactPathCandidate, ArtifactPathCommand, PaintMode,
    PathArtifactScanState, filter_artifact_paths,
};
use super::tagged::TaggedImageAppearance;
use super::*;
use crate::error::WarningKind;
use crate::formats::pdf::types::{RawImage, RawPath, TextSpan};
use crate::model::BoundingBox;
use crate::model::{PageDimensions, RawBox, TextDirection, ValueOrigin};
use crate::traits::FormatDecoder;
use pdf_oxide::extractors::text::{ArtifactType, PaginationSubtype};
use pdf_oxide::extractors::{ColorSpace, ImageData, PdfImage};
use pdf_oxide::geometry::{Point as PdfPoint, Rect};
use pdf_oxide::object::Object;
use std::collections::HashMap;

#[path = "tests/artifact_paths.rs"]
mod artifact_paths;
#[path = "tests/artifact_risk.rs"]
mod artifact_risk;
#[path = "tests/artifact_text.rs"]
mod artifact_text;
#[path = "tests/foundation.rs"]
mod foundation;

fn dummy_internal_span(text: &str, bbox: BoundingBox, mcid: Option<u32>) -> TextSpan {
    TextSpan {
        content: text.to_string(),
        bbox,
        font_name: "Helvetica".to_string(),
        font_size: 12.0,
        is_bold: false,
        is_italic: false,
        color: None,
        text_direction: TextDirection::LeftToRight,
        text_direction_origin: ValueOrigin::Observed,
        mcid,
        char_bboxes: Vec::new(),
    }
}

fn build_pdf_with_catalog_lang(lang: &str) -> Vec<u8> {
    let escaped_lang = lang
        .replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)");
    let content_stream = "BT /F1 12 Tf 72 720 Td (Hello) Tj ET";
    let content_length = content_stream.len();

    let mut pdf = String::new();
    pdf.push_str("%PDF-1.4\n");

    let obj1_offset = pdf.len();
    pdf.push_str(&format!(
        "1 0 obj\n<< /Type /Catalog /Pages 2 0 R /Lang ({}) >>\nendobj\n",
        escaped_lang
    ));

    let obj2_offset = pdf.len();
    pdf.push_str("2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");

    let obj3_offset = pdf.len();
    pdf.push_str(
        "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n",
    );

    let obj4_offset = pdf.len();
    pdf.push_str(&format!(
        "4 0 obj\n<< /Length {} >>\nstream\n{}\nendstream\nendobj\n",
        content_length, content_stream
    ));

    let obj5_offset = pdf.len();
    pdf.push_str("5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n");

    let xref_offset = pdf.len();
    pdf.push_str("xref\n0 6\n");
    pdf.push_str("0000000000 65535 f \n");
    pdf.push_str(&format!("{:010} 00000 n \n", obj1_offset));
    pdf.push_str(&format!("{:010} 00000 n \n", obj2_offset));
    pdf.push_str(&format!("{:010} 00000 n \n", obj3_offset));
    pdf.push_str(&format!("{:010} 00000 n \n", obj4_offset));
    pdf.push_str(&format!("{:010} 00000 n \n", obj5_offset));

    pdf.push_str(&format!(
        "trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
        xref_offset
    ));

    pdf.into_bytes()
}
