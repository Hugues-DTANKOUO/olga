use std::collections::HashMap;

use pdf_oxide::PdfDocument;
use pdf_oxide::object::{Object, ObjectRef};

// ---------------------------------------------------------------------------
// Cached page data — single-pass page tree traversal
// ---------------------------------------------------------------------------

/// Pre-computed page data collected in a single page tree traversal.
/// Eliminates O(n²) repeated tree walks for annotations and path scanning.
#[derive(Clone)]
pub(super) struct CachedPageData {
    /// The raw page dictionary (needed by link annotation extraction for /Annots).
    pub page_dict: HashMap<String, Object>,
    /// The resolved /Resources object (with inheritance applied).
    pub resources: Object,
}

/// Traverse the PDF page tree once and collect page dictionaries + resolved
/// resources for every page. Returns `Vec<CachedPageData>` indexed by page number.
pub(super) fn build_page_cache(
    doc: &mut PdfDocument,
    page_count: usize,
) -> Result<Vec<CachedPageData>, String> {
    let catalog = doc.catalog().map_err(|err| err.to_string())?;
    let catalog_dict = catalog
        .as_dict()
        .ok_or_else(|| "PDF catalog is not a dictionary".to_string())?;
    let pages_ref = catalog_dict
        .get("Pages")
        .and_then(Object::as_reference)
        .ok_or_else(|| "PDF catalog is missing a valid /Pages reference".to_string())?;

    let mut result = Vec::with_capacity(page_count);
    collect_all_pages(doc, pages_ref, None, &mut result)?;
    Ok(result)
}

fn collect_all_pages(
    doc: &mut PdfDocument,
    node_ref: ObjectRef,
    inherited_resources: Option<Object>,
    out: &mut Vec<CachedPageData>,
) -> Result<(), String> {
    let node = doc.load_object(node_ref).map_err(|err| err.to_string())?;
    let node_dict = node
        .as_dict()
        .ok_or_else(|| format!("Page tree node {} is not a dictionary", node_ref))?;
    let node_type = node_dict
        .get("Type")
        .and_then(Object::as_name)
        .unwrap_or_default();

    match node_type {
        "Page" => {
            let resources = node_dict
                .get("Resources")
                .cloned()
                .or(inherited_resources)
                .unwrap_or_else(|| Object::Dictionary(HashMap::new()));
            let resolved = resolve_object(doc, &resources).unwrap_or(resources);
            out.push(CachedPageData {
                page_dict: node_dict.clone(),
                resources: resolved,
            });
            Ok(())
        }
        "Pages" | "" => {
            let inherited_resources = node_dict.get("Resources").cloned().or(inherited_resources);
            let kids = node_dict
                .get("Kids")
                .and_then(Object::as_array)
                .ok_or_else(|| format!("Pages node {} is missing a valid /Kids array", node_ref))?;

            for kid in kids {
                let Some(kid_ref) = kid.as_reference() else {
                    continue;
                };
                collect_all_pages(doc, kid_ref, inherited_resources.clone(), out)?;
            }
            Ok(())
        }
        other => Err(format!(
            "Page tree node {} has unsupported /Type '{}'",
            node_ref, other
        )),
    }
}

pub(super) fn load_page_resources(
    doc: &mut PdfDocument,
    page_idx: usize,
) -> Result<Object, String> {
    let catalog = doc.catalog().map_err(|err| err.to_string())?;
    let catalog_dict = catalog
        .as_dict()
        .ok_or_else(|| "PDF catalog is not a dictionary".to_string())?;
    let pages_ref = catalog_dict
        .get("Pages")
        .and_then(Object::as_reference)
        .ok_or_else(|| "PDF catalog is missing a valid /Pages reference".to_string())?;

    let mut current_index = 0usize;
    find_page_resources_in_tree(doc, pages_ref, page_idx, &mut current_index, None)?
        .ok_or_else(|| format!("Page {} was not found in the PDF page tree", page_idx))
}

pub(super) fn resolve_marked_content_properties(
    doc: &mut PdfDocument,
    properties: &Object,
    resources: &Object,
) -> Option<Object> {
    match properties {
        Object::Name(name) => resolve_named_resource_entry(doc, resources, "Properties", name)
            .map(|(object, _)| object),
        _ => resolve_object(doc, properties),
    }
}

pub(super) fn resolve_named_xobject(
    doc: &mut PdfDocument,
    resources: &Object,
    name: &str,
) -> Option<(Object, Option<ObjectRef>)> {
    resolve_named_resource_entry(doc, resources, "XObject", name)
}

pub(super) fn resolve_object(doc: &mut PdfDocument, object: &Object) -> Option<Object> {
    match object.as_reference() {
        Some(reference) => doc.load_object(reference).ok(),
        None => Some(object.clone()),
    }
}

fn find_page_resources_in_tree(
    doc: &mut PdfDocument,
    node_ref: ObjectRef,
    target_index: usize,
    current_index: &mut usize,
    inherited_resources: Option<Object>,
) -> Result<Option<Object>, String> {
    let node = doc.load_object(node_ref).map_err(|err| err.to_string())?;
    let node_dict = node
        .as_dict()
        .ok_or_else(|| format!("Page tree node {} is not a dictionary", node_ref))?;
    let node_type = node_dict
        .get("Type")
        .and_then(Object::as_name)
        .unwrap_or_default();

    match node_type {
        "Page" => {
            if *current_index != target_index {
                *current_index += 1;
                return Ok(None);
            }

            let resources = node_dict
                .get("Resources")
                .cloned()
                .or(inherited_resources)
                .unwrap_or_else(|| Object::Dictionary(std::collections::HashMap::new()));

            Ok(Some(resolve_object(doc, &resources).unwrap_or(resources)))
        }
        "Pages" | "" => {
            let inherited_resources = node_dict.get("Resources").cloned().or(inherited_resources);
            let kids = node_dict
                .get("Kids")
                .and_then(Object::as_array)
                .ok_or_else(|| format!("Pages node {} is missing a valid /Kids array", node_ref))?;

            for kid in kids {
                let Some(kid_ref) = kid.as_reference() else {
                    continue;
                };

                if let Some(resources) = find_page_resources_in_tree(
                    doc,
                    kid_ref,
                    target_index,
                    current_index,
                    inherited_resources.clone(),
                )? {
                    return Ok(Some(resources));
                }
            }

            Ok(None)
        }
        other => Err(format!(
            "Page tree node {} has unsupported /Type '{}'",
            node_ref, other
        )),
    }
}

fn resolve_named_resource_entry(
    doc: &mut PdfDocument,
    resources: &Object,
    category: &str,
    name: &str,
) -> Option<(Object, Option<ObjectRef>)> {
    let resources_dict = resources.as_dict()?;
    let category_obj = resources_dict.get(category)?;
    let resolved_category = resolve_object(doc, category_obj)?;
    let category_dict = resolved_category.as_dict()?;
    let entry = category_dict.get(name)?;
    let reference = entry.as_reference();
    let resolved_entry = resolve_object(doc, entry)?;
    Some((resolved_entry, reference))
}
