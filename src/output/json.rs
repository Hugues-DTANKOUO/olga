//! JSON renderer — converts a [`StructureResult`] into the Olga JSON schema.
//!
//! The JSON output is designed for programmatic consumption by downstream
//! indexing and search pipelines. Key features:
//!
//! - **Hierarchical elements** with optional `children` arrays
//! - **Bounding boxes** on every element (normalized 0–1, resolution-independent)
//! - **Confidence scores** per element (1.0 = format-derived, lower = heuristic)
//! - **Provenance tracking** — `"format-derived"`, `"heuristic:TableDetector"`, etc.
//! - **Dual table representation** — simple `headers`/`data` arrays for quick access,
//!   plus detailed `cells` array with per-cell bbox and span info
//! - **Deterministic element IDs** — `"e-{page}-{index}"` for caching and cross-refs

#[path = "json/helpers.rs"]
mod helpers;
#[path = "json/nodes.rs"]
mod nodes;
#[path = "json/schema.rs"]
mod schema;
#[path = "json/tables.rs"]
mod tables;
#[cfg(test)]
#[path = "json/tests.rs"]
mod tests;

use serde_json::{Value, json};

use crate::structure::StructureResult;

use self::nodes::{ElementCounter, render_node};
use self::schema::{render_pages, render_source};

/// Render a [`StructureResult`] as a JSON [`Value`].
///
/// The returned value can be serialized with `serde_json::to_string_pretty()`
/// or `serde_json::to_string()` depending on whether pretty-printing is desired.
pub fn render(result: &StructureResult) -> Value {
    let mut counter = ElementCounter::default();

    let source = render_source(&result.metadata);
    let pages = render_pages(&result.metadata);

    let elements: Vec<Value> = result
        .root
        .children
        .iter()
        .map(|child| render_node(child, &mut counter))
        .collect();

    let mut doc = json!({
        "olga_version": env!("CARGO_PKG_VERSION"),
        "source": source,
        "pages": pages,
        "elements": elements,
    });

    if !result.warnings.is_empty() {
        let warnings: Vec<Value> = result
            .warnings
            .iter()
            .map(|w| {
                json!({
                    "kind": format!("{:?}", w.kind),
                    "message": w.message,
                    "page": w.page,
                })
            })
            .collect();
        if let Value::Object(ref mut map) = doc {
            map.insert("warnings".to_string(), Value::Array(warnings));
        }
    }

    doc
}
