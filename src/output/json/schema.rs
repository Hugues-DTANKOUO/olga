use serde_json::{Value, json};

use crate::model::DocumentMetadata;

pub(super) fn render_source(meta: &DocumentMetadata) -> Value {
    json!({
        "format": meta.format.to_string(),
        "page_count": meta.page_count,
        "file_size": meta.file_size,
        "title": meta.title,
        "encrypted": meta.encrypted,
    })
}

pub(super) fn render_pages(meta: &DocumentMetadata) -> Vec<Value> {
    meta.page_dimensions
        .iter()
        .enumerate()
        .map(|(i, dims)| {
            json!({
                "page": i,
                "width_pt": dims.effective_width,
                "height_pt": dims.effective_height,
                "rotation": dims.rotation,
            })
        })
        .collect()
}
