use serde_json::{Value, json};

use crate::model::{DocumentNode, NodeKind};

use super::helpers::{provenance_string, round2, round4};
use super::tables::{TableContext, render_table};

#[derive(Default)]
pub(super) struct ElementCounter {
    /// Per-page counters: page → next index.
    page_counters: std::collections::HashMap<u32, u32>,
}

impl ElementCounter {
    pub(super) fn next_id(&mut self, page: u32) -> String {
        let idx = self.page_counters.entry(page).or_insert(0);
        let id = format!("e-{}-{}", page, idx);
        *idx += 1;
        id
    }
}

// ---------------------------------------------------------------------------
// Node rendering
// ---------------------------------------------------------------------------

pub(super) fn render_node(node: &DocumentNode, counter: &mut ElementCounter) -> Value {
    let page = node.source_pages.start;
    let id = counter.next_id(page);

    let bbox = json!({
        "x": round4(node.bbox.x),
        "y": round4(node.bbox.y),
        "w": round4(node.bbox.width),
        "h": round4(node.bbox.height),
    });

    let source_str = provenance_string(&node.structure_source);

    let mut obj = match &node.kind {
        NodeKind::Document => {
            json!({
                "id": id,
                "type": "document",
                "bbox": bbox,
                "page": page,
                "confidence": round2(node.confidence),
                "source": source_str,
            })
        }
        NodeKind::Section { level } => {
            json!({
                "id": id,
                "type": "section",
                "level": level,
                "bbox": bbox,
                "page": page,
                "confidence": round2(node.confidence),
                "source": source_str,
            })
        }
        NodeKind::Heading { level, text } => {
            json!({
                "id": id,
                "type": "heading",
                "level": level,
                "text": text,
                "bbox": bbox,
                "page": page,
                "confidence": round2(node.confidence),
                "source": source_str,
            })
        }
        NodeKind::Paragraph { text } => {
            json!({
                "id": id,
                "type": "paragraph",
                "text": text,
                "bbox": bbox,
                "page": page,
                "confidence": round2(node.confidence),
                "source": source_str,
            })
        }
        NodeKind::Table { rows, cols } => {
            let ctx = TableContext {
                id,
                bbox,
                page,
                source_str,
            };
            return render_table(node, *rows, *cols, ctx, counter);
        }
        NodeKind::TableRow => {
            // TableRows are inlined into the table's cells array — should not
            // appear at top level, but handle gracefully.
            json!({
                "id": id,
                "type": "table_row",
                "bbox": bbox,
                "page": page,
                "confidence": round2(node.confidence),
                "source": source_str,
            })
        }
        NodeKind::TableCell {
            row,
            col,
            rowspan,
            colspan,
            text,
        } => {
            json!({
                "id": id,
                "type": "table_cell",
                "row": row,
                "col": col,
                "rowspan": rowspan,
                "colspan": colspan,
                "text": text,
                "bbox": bbox,
                "page": page,
                "confidence": round2(node.confidence),
                "source": source_str,
            })
        }
        NodeKind::List { ordered } => {
            json!({
                "id": id,
                "type": "list",
                "ordered": ordered,
                "bbox": bbox,
                "page": page,
                "confidence": round2(node.confidence),
                "source": source_str,
            })
        }
        NodeKind::ListItem { text } => {
            json!({
                "id": id,
                "type": "list_item",
                "text": text,
                "bbox": bbox,
                "page": page,
                "confidence": round2(node.confidence),
                "source": source_str,
            })
        }
        NodeKind::Image { alt_text, format } => {
            json!({
                "id": id,
                "type": "image",
                "format": format,
                "alt_text": alt_text,
                "bbox": bbox,
                "page": page,
                "confidence": round2(node.confidence),
                "source": source_str,
            })
        }
        NodeKind::CodeBlock { language, text } => {
            json!({
                "id": id,
                "type": "code_block",
                "language": language,
                "text": text,
                "bbox": bbox,
                "page": page,
                "confidence": round2(node.confidence),
                "source": source_str,
            })
        }
        NodeKind::BlockQuote { text } => {
            json!({
                "id": id,
                "type": "block_quote",
                "text": text,
                "bbox": bbox,
                "page": page,
                "confidence": round2(node.confidence),
                "source": source_str,
            })
        }
        NodeKind::PageHeader { text } => {
            json!({
                "id": id,
                "type": "page_header",
                "text": text,
                "bbox": bbox,
                "page": page,
                "confidence": round2(node.confidence),
                "source": source_str,
            })
        }
        NodeKind::PageFooter { text } => {
            json!({
                "id": id,
                "type": "page_footer",
                "text": text,
                "bbox": bbox,
                "page": page,
                "confidence": round2(node.confidence),
                "source": source_str,
            })
        }
        NodeKind::Footnote { id: fn_id, text } => {
            json!({
                "id": id,
                "type": "footnote",
                "footnote_id": fn_id,
                "text": text,
                "bbox": bbox,
                "page": page,
                "confidence": round2(node.confidence),
                "source": source_str,
            })
        }
        NodeKind::AlignedLine { spans } => {
            // Build flat text representation.
            let mut flat = String::new();
            let mut cursor: usize = 0;
            for span in spans {
                if span.col_start > cursor {
                    for _ in 0..(span.col_start - cursor) {
                        flat.push(' ');
                    }
                }
                flat.push_str(&span.text);
                cursor = span.col_start + span.text.chars().count();
            }

            let mut obj = json!({
                "id": id,
                "type": "aligned_line",
                "text": flat.trim_end(),
                "bbox": bbox,
                "page": page,
            });

            // Only include spans when at least one carries formatting
            // (bold, italic, or link). Plain lines save ~60% of JSON size
            // by omitting the redundant per-word breakdown.
            let has_formatting = spans
                .iter()
                .any(|s| s.is_bold || s.is_italic || s.link_url.is_some());
            if has_formatting {
                let span_values: Vec<Value> = spans
                    .iter()
                    .map(|s| {
                        let mut sobj = json!({ "col": s.col_start, "text": s.text });
                        if s.is_bold {
                            sobj["bold"] = json!(true);
                        }
                        if s.is_italic {
                            sobj["italic"] = json!(true);
                        }
                        if let Some(ref url) = s.link_url {
                            sobj["link"] = json!(url);
                        }
                        sobj
                    })
                    .collect();
                obj["spans"] = Value::Array(span_values);
            }

            obj["confidence"] = json!(round2(node.confidence));
            obj["source"] = json!(source_str);

            obj
        }
    };

    // Add children if present (for non-table nodes).
    if !node.children.is_empty() {
        let children: Vec<Value> = node
            .children
            .iter()
            .map(|c| render_node(c, counter))
            .collect();
        if let Value::Object(ref mut map) = obj {
            map.insert("children".to_string(), Value::Array(children));
        }
    }

    strip_defaults(&mut obj);
    obj
}

/// Remove fields that carry default/boilerplate values to reduce JSON size.
/// - `confidence` = 1.0 (format-derived) is the common case → omit
/// - `source` = "format-derived" is the common case → omit
fn strip_defaults(obj: &mut Value) {
    if let Some(map) = obj.as_object_mut() {
        if map.get("confidence").and_then(Value::as_f64) == Some(1.0) {
            map.remove("confidence");
        }
        if map
            .get("source")
            .and_then(Value::as_str)
            .is_some_and(|s| s == "format-derived")
        {
            map.remove("source");
        }
    }
}
