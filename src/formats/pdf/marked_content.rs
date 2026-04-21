use pdf_oxide::PdfDocument;
use pdf_oxide::extractors::text::{ArtifactType, PaginationSubtype};
use pdf_oxide::object::Object;

use super::resources::resolve_marked_content_properties;

#[derive(Debug, Clone, Default)]
pub(super) struct MarkedContentContext {
    pub(super) mcid: Option<u32>,
    pub(super) is_artifact: bool,
    pub(super) artifact_type: Option<ArtifactType>,
}

pub(super) fn marked_content_context_for_tag(tag: &str) -> MarkedContentContext {
    MarkedContentContext {
        is_artifact: tag.eq_ignore_ascii_case("Artifact"),
        ..MarkedContentContext::default()
    }
}

pub(super) fn build_marked_content_context(
    doc: &mut PdfDocument,
    tag: &str,
    properties: &Object,
    resources: &Object,
) -> MarkedContentContext {
    let resolved_properties = resolve_marked_content_properties(doc, properties, resources);
    let mcid = resolved_properties
        .as_ref()
        .and_then(extract_mcid_from_properties_dict);
    let artifact_type = classify_marked_content_artifact(tag, resolved_properties.as_ref());

    MarkedContentContext {
        mcid,
        is_artifact: tag.eq_ignore_ascii_case("Artifact"),
        artifact_type,
    }
}

pub(super) fn active_marked_content_mcid(
    marked_content_stack: &[MarkedContentContext],
) -> Option<u32> {
    marked_content_stack.iter().rev().find_map(|ctx| ctx.mcid)
}

pub(super) fn active_marked_content_artifact(
    marked_content_stack: &[MarkedContentContext],
) -> Option<&MarkedContentContext> {
    marked_content_stack
        .iter()
        .rev()
        .find(|ctx| ctx.is_artifact)
}

pub(super) fn extract_mcid_from_properties_dict(properties: &Object) -> Option<u32> {
    properties
        .as_dict()
        .and_then(|dict| dict.get("MCID"))
        .and_then(Object::as_integer)
        .and_then(|mcid| u32::try_from(mcid).ok())
}

pub(super) fn classify_marked_content_artifact(
    tag: &str,
    properties: Option<&Object>,
) -> Option<ArtifactType> {
    if !tag.eq_ignore_ascii_case("Artifact") {
        return None;
    }

    properties.and_then(extract_artifact_type_from_properties_dict)
}

pub(super) fn extract_artifact_type_from_properties_dict(
    properties: &Object,
) -> Option<ArtifactType> {
    let properties = properties.as_dict()?;
    let artifact_type_name = properties
        .get("Type")
        .and_then(Object::as_name)
        .map(|name| name.to_ascii_lowercase());
    let subtype_name = properties
        .get("Subtype")
        .and_then(Object::as_name)
        .map(|name| name.to_ascii_lowercase());

    match artifact_type_name.as_deref() {
        Some("pagination") => Some(ArtifactType::Pagination(match subtype_name.as_deref() {
            Some("header") => PaginationSubtype::Header,
            Some("footer") => PaginationSubtype::Footer,
            Some("watermark") => PaginationSubtype::Watermark,
            Some("pagenumber") | Some("page") => PaginationSubtype::PageNumber,
            _ => PaginationSubtype::Other,
        })),
        Some("layout") => Some(ArtifactType::Layout),
        Some("page") => Some(ArtifactType::Page),
        Some("background") => Some(ArtifactType::Background),
        None => match subtype_name.as_deref() {
            Some("header") => Some(ArtifactType::Pagination(PaginationSubtype::Header)),
            Some("footer") => Some(ArtifactType::Pagination(PaginationSubtype::Footer)),
            Some("watermark") => Some(ArtifactType::Pagination(PaginationSubtype::Watermark)),
            _ => None,
        },
        _ => None,
    }
}

pub(super) fn describe_artifact_kind(artifact_type: Option<&ArtifactType>) -> &'static str {
    match artifact_type {
        Some(ArtifactType::Pagination(PaginationSubtype::Header)) => "pagination/header",
        Some(ArtifactType::Pagination(PaginationSubtype::Footer)) => "pagination/footer",
        Some(ArtifactType::Pagination(PaginationSubtype::Watermark)) => "pagination/watermark",
        Some(ArtifactType::Pagination(PaginationSubtype::PageNumber)) => "pagination/page-number",
        Some(ArtifactType::Pagination(PaginationSubtype::Other)) => "pagination/other",
        Some(ArtifactType::Layout) => "layout",
        Some(ArtifactType::Page) => "page",
        Some(ArtifactType::Background) => "background",
        None => "artifact",
    }
}
