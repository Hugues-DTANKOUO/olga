use super::*;
use crate::error::Warning;
use crate::model::{
    BoundingBox, DocumentFormat, DocumentMetadata, HintKind, PaginationBasis, PaginationProvenance,
    Primitive, PrimitiveKind, SemanticHint,
};
use crate::traits::DecodeResult;

#[path = "tests/assembly.rs"]
mod assembly;
#[path = "tests/basic.rs"]
mod basic;
#[path = "tests/builder.rs"]
mod builder;
#[path = "tests/config.rs"]
mod config;
#[path = "tests/heuristics.rs"]
mod heuristics;
#[path = "tests/warnings.rs"]
mod warnings;

fn text_prim(page: u32, y: f32, content: &str, order: u64) -> Primitive {
    Primitive::observed(
        PrimitiveKind::Text {
            content: content.to_string(),
            font_size: 12.0,
            font_name: "Arial".to_string(),
            is_bold: false,
            is_italic: false,
            color: None,
            char_positions: None,
            text_direction: Default::default(),
            text_direction_origin: crate::model::provenance::ValueOrigin::Observed,
            lang: None,
            lang_origin: None,
        },
        BoundingBox::new(0.05, y, 0.9, 0.02),
        page,
        order,
    )
}

fn text_prim_sized(
    page: u32,
    y: f32,
    font_size: f32,
    bold: bool,
    content: &str,
    order: u64,
) -> Primitive {
    Primitive::observed(
        PrimitiveKind::Text {
            content: content.to_string(),
            font_size,
            font_name: "Arial".to_string(),
            is_bold: bold,
            is_italic: false,
            color: None,
            char_positions: None,
            text_direction: Default::default(),
            text_direction_origin: crate::model::provenance::ValueOrigin::Observed,
            lang: None,
            lang_origin: None,
        },
        BoundingBox::new(0.05, y, 0.9, 0.02),
        page,
        order,
    )
}

fn hinted(prim: Primitive, kind: HintKind) -> Primitive {
    prim.with_hint(SemanticHint::from_format(kind))
}

fn test_metadata() -> DocumentMetadata {
    DocumentMetadata {
        format: DocumentFormat::Pdf,
        page_count: 1,
        page_count_provenance: PaginationProvenance::observed(PaginationBasis::PhysicalPages),
        page_dimensions: vec![],
        encrypted: false,
        text_extraction_allowed: true,
        file_size: 1024,
        title: None,
    }
}

fn decode_result(primitives: Vec<Primitive>) -> DecodeResult {
    DecodeResult {
        metadata: test_metadata(),
        primitives,
        warnings: Vec::new(),
        images: Vec::new(),
    }
}

fn decode_result_with_warnings(primitives: Vec<Primitive>, warnings: Vec<Warning>) -> DecodeResult {
    DecodeResult {
        metadata: test_metadata(),
        primitives,
        warnings,
        images: Vec::new(),
    }
}
