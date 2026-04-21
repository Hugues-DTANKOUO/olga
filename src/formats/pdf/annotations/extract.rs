use pdf_oxide::PdfDocument;
use pdf_oxide::object::Object;

use crate::model::PageDimensions;
use crate::model::geometry::BoundingBox;

use super::super::resources::resolve_object;

/// A link annotation extracted from a PDF page.
#[derive(Debug, Clone)]
pub(crate) struct LinkAnnotation {
    /// Normalized bounding box (0.0–1.0, Y=0 at top).
    pub bbox: BoundingBox,
    /// Target URL.
    pub url: String,
}

pub(super) fn extract_link_annotations_inner(
    doc: &mut PdfDocument,
    page_idx: usize,
    page_dims: &PageDimensions,
    cached_page_dict: Option<&std::collections::HashMap<String, Object>>,
) -> Result<Vec<LinkAnnotation>, String> {
    let owned_dict;
    let page_dict = match cached_page_dict {
        Some(d) => d,
        None => {
            owned_dict = find_page_dict(doc, page_idx)?;
            &owned_dict
        }
    };

    let annots_obj = page_dict
        .get("Annots")
        .ok_or_else(|| "No /Annots on page".to_string())?;

    let annots_resolved =
        resolve_object(doc, annots_obj).ok_or_else(|| "Failed to resolve /Annots".to_string())?;

    let annots_array = annots_resolved
        .as_array()
        .ok_or_else(|| "/Annots is not an array".to_string())?;

    let mut links = Vec::new();

    for annot_obj in annots_array {
        let annot = match annot_obj.as_reference() {
            Some(r) => match doc.load_object(r) {
                Ok(obj) => obj,
                Err(_) => continue,
            },
            None => annot_obj.clone(),
        };

        let Some(annot_dict) = annot.as_dict() else {
            continue;
        };

        let subtype = annot_dict.get("Subtype").and_then(Object::as_name);
        if subtype != Some("Link") {
            continue;
        }

        let Some(url) = extract_url_from_action(doc, annot_dict) else {
            continue;
        };

        let Some(rect) = extract_rect(annot_dict) else {
            continue;
        };

        let (x0, y0, x1, y1) = rect;
        let raw_x = x0.min(x1);
        let raw_y = y0.min(y1);
        let raw_width = (x1 - x0).abs();
        let raw_height = (y1 - y0).abs();

        let bbox = page_dims.normalize_bbox(raw_x, raw_y, raw_width, raw_height, true);
        links.push(LinkAnnotation { bbox, url });
    }

    Ok(links)
}

fn extract_url_from_action(
    doc: &mut PdfDocument,
    annot_dict: &std::collections::HashMap<String, Object>,
) -> Option<String> {
    if let Some(action_obj) = annot_dict.get("A") {
        let action = resolve_object(doc, action_obj)?;
        let action_dict = action.as_dict()?;

        if let Some(uri_obj) = action_dict.get("URI") {
            return extract_string_value(uri_obj);
        }
    }

    if let Some(uri_obj) = annot_dict.get("URI") {
        return extract_string_value(uri_obj);
    }

    None
}

fn extract_string_value(obj: &Object) -> Option<String> {
    if let Some(name) = obj.as_name() {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    if let Some(bytes) = obj.as_string() {
        let text = String::from_utf8_lossy(bytes);
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    None
}

fn extract_rect(
    annot_dict: &std::collections::HashMap<String, Object>,
) -> Option<(f32, f32, f32, f32)> {
    let rect_obj = annot_dict.get("Rect")?;
    let rect_array = rect_obj.as_array()?;
    if rect_array.len() < 4 {
        return None;
    }

    let x0 = obj_to_f32(&rect_array[0])?;
    let y0 = obj_to_f32(&rect_array[1])?;
    let x1 = obj_to_f32(&rect_array[2])?;
    let y1 = obj_to_f32(&rect_array[3])?;

    Some((x0, y0, x1, y1))
}

fn obj_to_f32(obj: &Object) -> Option<f32> {
    if let Some(f) = obj.as_real() {
        return Some(f as f32);
    }
    if let Some(i) = obj.as_integer() {
        return Some(i as f32);
    }
    None
}

fn find_page_dict(
    doc: &mut PdfDocument,
    page_idx: usize,
) -> Result<std::collections::HashMap<String, Object>, String> {
    let catalog = doc.catalog().map_err(|e| e.to_string())?;
    let catalog_dict = catalog
        .as_dict()
        .ok_or_else(|| "Catalog is not a dictionary".to_string())?;
    let pages_ref = catalog_dict
        .get("Pages")
        .and_then(Object::as_reference)
        .ok_or_else(|| "Missing /Pages reference".to_string())?;

    let mut current = 0usize;
    find_page_in_tree(doc, pages_ref, page_idx, &mut current)?
        .ok_or_else(|| format!("Page {} not found in page tree", page_idx))
}

fn find_page_in_tree(
    doc: &mut PdfDocument,
    node_ref: pdf_oxide::object::ObjectRef,
    target: usize,
    current: &mut usize,
) -> Result<Option<std::collections::HashMap<String, Object>>, String> {
    let node = doc.load_object(node_ref).map_err(|e| e.to_string())?;
    let node_dict = node
        .as_dict()
        .ok_or_else(|| "Page tree node is not a dict".to_string())?;
    let node_type = node_dict
        .get("Type")
        .and_then(Object::as_name)
        .unwrap_or_default();

    match node_type {
        "Page" => {
            if *current == target {
                Ok(Some(node_dict.clone()))
            } else {
                *current += 1;
                Ok(None)
            }
        }
        "Pages" | "" => {
            let kids = node_dict
                .get("Kids")
                .and_then(Object::as_array)
                .ok_or_else(|| "Missing /Kids".to_string())?;

            for kid in kids {
                let Some(kid_ref) = kid.as_reference() else {
                    continue;
                };
                if let Some(dict) = find_page_in_tree(doc, kid_ref, target, current)? {
                    return Ok(Some(dict));
                }
            }
            Ok(None)
        }
        other => Err(format!("Unsupported page tree node type: {}", other)),
    }
}
