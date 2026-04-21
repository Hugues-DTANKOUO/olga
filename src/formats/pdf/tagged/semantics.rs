use std::collections::HashMap;

use pdf_oxide::object::Object;
use pdf_oxide::structure::{StructElem, StructType};

use crate::model::{HintKind, SemanticHint, ValueOrigin};

use super::{
    BlockSemantic, ListContext, TableCellContext, TableState, TaggedContentMetadata,
    TraversalState, normalize_text_value,
};

pub(super) fn section_start_hint_for(elem: &StructElem) -> Option<SemanticHint> {
    match elem.struct_type {
        StructType::Part | StructType::Art | StructType::Sect | StructType::Div => {
            Some(SemanticHint::from_format(HintKind::SectionStart {
                title: extract_section_title(elem),
            }))
        }
        _ => None,
    }
}

pub(super) fn section_end_hint_for(elem: &StructElem) -> Option<SemanticHint> {
    match elem.struct_type {
        StructType::Part | StructType::Art | StructType::Sect | StructType::Div => {
            Some(SemanticHint::from_format(HintKind::SectionEnd))
        }
        _ => None,
    }
}

pub(super) fn block_semantic_for(struct_type: &StructType) -> Option<BlockSemantic> {
    if let Some(level) = struct_type.heading_level() {
        return Some(BlockSemantic::Heading(level));
    }

    match struct_type {
        StructType::P => Some(BlockSemantic::Paragraph),
        StructType::Quote => Some(BlockSemantic::BlockQuote),
        StructType::Code => Some(BlockSemantic::CodeBlock),
        StructType::Note | StructType::Annot => Some(BlockSemantic::Sidebar),
        _ => None,
    }
}

pub(super) fn allocate_cell_context(
    elem: &StructElem,
    table_state: Option<&mut TableState>,
) -> Option<TableCellContext> {
    let table_state = table_state?;
    let row_state = table_state.current_row.as_mut()?;
    let colspan = attr_u32(&elem.attributes, "ColSpan").max(1);
    let rowspan = attr_u32(&elem.attributes, "RowSpan").max(1);
    let col = row_state.next_col;
    row_state.next_col = row_state.next_col.saturating_add(colspan);

    Some(TableCellContext {
        row: row_state.row,
        col,
        rowspan,
        colspan,
        is_header: matches!(elem.struct_type, StructType::TH),
    })
}

pub(super) fn content_metadata_for(
    state: &TraversalState,
    lang: Option<String>,
    expansion_text: Option<String>,
    source_role: Option<&str>,
) -> TaggedContentMetadata {
    let mut metadata = TaggedContentMetadata {
        lang,
        lang_origin: Some(ValueOrigin::Observed),
        ..Default::default()
    };

    if metadata.lang.is_none() {
        metadata.lang_origin = None;
    }

    for hint in active_hints(state) {
        metadata.add_hint(hint);
    }

    if let Some(hint) = source_role_hint(source_role) {
        metadata.add_hint(hint);
    }

    if let Some(expansion_text) = expansion_text {
        metadata.add_hint(SemanticHint::from_format(HintKind::ExpandedText {
            text: expansion_text,
        }));
    }

    metadata
}

pub(super) fn list_context_for(elem: &StructElem, nesting_depth: usize) -> ListContext {
    let numbering = elem
        .attributes
        .get("ListNumbering")
        .or_else(|| elem.attributes.get("listnumbering"))
        .and_then(|value| {
            value.as_name().map(str::to_owned).or_else(|| {
                value
                    .as_string()
                    .map(|s| String::from_utf8_lossy(s).into_owned())
            })
        })
        .and_then(|value| normalize_text_value(Some(value.as_str())));

    let ordered = numbering
        .as_deref()
        .map(list_numbering_is_ordered)
        .unwrap_or(false);

    ListContext {
        depth: nesting_depth as u8,
        ordered,
        ordered_observed: numbering.is_some(),
    }
}

pub(super) fn link_hint_for(elem: &StructElem) -> Option<SemanticHint> {
    if !matches!(elem.struct_type, StructType::Link | StructType::Reference) {
        return None;
    }

    extract_link_target_from_attributes(&elem.attributes)
        .map(|url| SemanticHint::from_format(HintKind::Link { url }))
}

pub(super) fn image_alt_text_for(elem: &StructElem) -> Option<String> {
    if matches!(
        elem.struct_type,
        StructType::Figure | StructType::Formula | StructType::Form
    ) {
        normalize_text_value(elem.alt_text.as_deref())
    } else {
        None
    }
}

pub(super) fn add_hint_if_missing(hints: &mut Vec<SemanticHint>, hint: SemanticHint) {
    if !hints.contains(&hint) {
        hints.push(hint);
    }
}

pub(super) fn extract_language_from_attributes(
    attributes: &HashMap<String, Object>,
) -> Option<String> {
    attributes
        .get("Lang")
        .and_then(|obj| match obj {
            Object::String(bytes) => Some(String::from_utf8_lossy(bytes).into_owned()),
            _ => None,
        })
        .and_then(|value| normalize_text_value(Some(value.as_str())))
}

fn extract_section_title(elem: &StructElem) -> Option<String> {
    for key in ["Title", "title", "T", "t"] {
        if let Some(value) = elem.attributes.get(key) {
            if let Some(name) = value.as_name()
                && let Some(title) = normalize_text_value(Some(name))
            {
                return Some(title);
            }
            if let Some(bytes) = value.as_string()
                && let Some(title) =
                    normalize_text_value(Some(String::from_utf8_lossy(bytes).as_ref()))
            {
                return Some(title);
            }
        }
    }

    None
}

fn active_hints(state: &TraversalState) -> Vec<SemanticHint> {
    let mut hints = Vec::new();

    if let Some(cell) = state.active_cell {
        hints.push(if cell.is_header {
            SemanticHint::from_format(HintKind::TableHeader { col: cell.col })
        } else {
            SemanticHint::from_format(HintKind::TableCell {
                row: cell.row,
                col: cell.col,
                rowspan: cell.rowspan,
                colspan: cell.colspan,
            })
        });
    }

    if let Some(list_item) = state.active_list_item {
        let hint_kind = HintKind::ListItem {
            depth: list_item.depth,
            ordered: list_item.ordered,
            list_group: None,
        };
        hints.push(if list_item.ordered_observed {
            SemanticHint::from_format(hint_kind)
        } else {
            SemanticHint::from_heuristic(hint_kind, 0.75, "tagged-pdf-list-inference")
        });
    }

    if let Some(link_hint) = state.link_stack.last() {
        hints.push(link_hint.clone());
    }

    if let Some(block) = state.block_stack.last() {
        hints.push(match block {
            BlockSemantic::Heading(level) => {
                SemanticHint::from_format(HintKind::Heading { level: *level })
            }
            BlockSemantic::Paragraph => SemanticHint::from_format(HintKind::Paragraph),
            BlockSemantic::BlockQuote => SemanticHint::from_format(HintKind::BlockQuote),
            BlockSemantic::CodeBlock => SemanticHint::from_format(HintKind::CodeBlock),
            BlockSemantic::Sidebar => SemanticHint::from_format(HintKind::Sidebar),
        });
    }

    hints
}

fn source_role_hint(source_role: Option<&str>) -> Option<SemanticHint> {
    let source_role = source_role?;
    match source_role.to_ascii_lowercase().as_str() {
        "title" | "documenttitle" => Some(SemanticHint::from_format(HintKind::DocumentTitle)),
        "aside" | "sidebar" => Some(SemanticHint::from_format(HintKind::Sidebar)),
        "blockquote" => Some(SemanticHint::from_format(HintKind::BlockQuote)),
        "codeblock" => Some(SemanticHint::from_format(HintKind::CodeBlock)),
        "listitem" => Some(SemanticHint::from_heuristic(
            HintKind::ListItem {
                depth: 0,
                ordered: false,
                list_group: None,
            },
            0.7,
            "tagged-pdf-role-inference",
        )),
        _ => None,
    }
}

fn extract_link_target_from_attributes(attributes: &HashMap<String, Object>) -> Option<String> {
    for key in ["URI", "Url", "URL"] {
        if let Some(value) = attributes.get(key) {
            if let Some(name) = value.as_name()
                && let Some(url) = normalize_text_value(Some(name))
            {
                return Some(url);
            }
            if let Some(bytes) = value.as_string()
                && let Some(url) =
                    normalize_text_value(Some(String::from_utf8_lossy(bytes).as_ref()))
            {
                return Some(url);
            }
        }
    }
    None
}

fn list_numbering_is_ordered(numbering: &str) -> bool {
    !matches!(
        numbering.to_ascii_lowercase().as_str(),
        "none" | "disc" | "circle" | "square"
    )
}

fn attr_u32(attributes: &HashMap<String, Object>, key: &str) -> u32 {
    attributes
        .get(key)
        .or_else(|| attributes.get(&key.to_ascii_lowercase()))
        .and_then(Object::as_integer)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(1)
}
