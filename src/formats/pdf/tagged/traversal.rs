use std::collections::{HashMap, HashSet};

use pdf_oxide::structure::{StructChild, StructElem, StructTreeRoot, StructType};

use super::semantics::{
    allocate_cell_context, block_semantic_for, content_metadata_for,
    extract_language_from_attributes, image_alt_text_for, link_hint_for, list_context_for,
    section_end_hint_for, section_start_hint_for,
};
use super::*;

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct TaggedSupportSummary {
    pub(super) alt_text_without_mcid: u32,
    pub(super) expansion_without_mcid: u32,
    pub(super) actual_text_without_mcid: u32,
    pub(super) object_ref_count: u32,
}

pub(super) fn build_tagged_pdf_context(
    struct_tree: &StructTreeRoot,
    use_structure_reading_order: bool,
) -> (TaggedPdfContext, TaggedSupportSummary) {
    let mut page_contexts = HashMap::new();
    let mut semantic_nodes = Vec::new();
    let mut state = TraversalState::default();
    let mut summary = TaggedSupportSummary::default();

    for root in &struct_tree.root_elements {
        traverse_structure_element(
            root,
            &mut state,
            None,
            None,
            &mut page_contexts,
            &mut semantic_nodes,
            &mut summary,
        );
    }

    (
        TaggedPdfContext {
            page_contexts,
            semantic_nodes,
            use_structure_reading_order,
        },
        summary,
    )
}

fn traverse_structure_element(
    elem: &StructElem,
    state: &mut TraversalState,
    inherited_lang: Option<String>,
    parent_node_id: Option<TaggedNodeId>,
    page_contexts: &mut HashMap<u32, TaggedPageContext>,
    semantic_nodes: &mut Vec<TaggedSemanticNode>,
    summary: &mut TaggedSupportSummary,
) -> TaggedNodeId {
    let pushed_block = block_semantic_for(&elem.struct_type);
    if let Some(block) = pushed_block {
        state.block_stack.push(block);
    }

    let pushed_list = if matches!(elem.struct_type, StructType::L) {
        state
            .list_stack
            .push(list_context_for(elem, state.list_stack.len()));
        true
    } else {
        false
    };

    let previous_list_item = if matches!(elem.struct_type, StructType::LI) {
        let previous = state.active_list_item;
        state.active_list_item = state.list_stack.last().copied();
        previous
    } else {
        None
    };

    let pushed_link = if let Some(link_hint) = link_hint_for(elem) {
        state.link_stack.push(link_hint);
        true
    } else {
        false
    };

    let pushed_table = matches!(elem.struct_type, StructType::Table);
    if pushed_table {
        state.table_stack.push(TableState::new());
    }

    let previous_row = if matches!(elem.struct_type, StructType::TR) {
        state.table_stack.last_mut().and_then(|table| {
            let previous = table.current_row;
            table.current_row = Some(RowState {
                row: table.next_row,
                next_col: 0,
            });
            table.next_row += 1;
            previous
        })
    } else {
        None
    };

    let previous_cell = state.active_cell;
    if matches!(elem.struct_type, StructType::TH | StructType::TD) {
        state.active_cell = allocate_cell_context(elem, state.table_stack.last_mut());
    }
    let unwind_state = TraversalUnwindState {
        pushed_block,
        pushed_list,
        pushed_link,
        previous_list_item,
        pushed_table,
        previous_row,
        previous_cell,
    };

    let current_lang = extract_language_from_attributes(&elem.attributes).or(inherited_lang);
    let image_alt_text = image_alt_text_for(elem);
    let actual_text = normalize_text_value(elem.actual_text.as_deref());
    let expansion_text = normalize_text_value(elem.expansion.as_deref());
    let descendant_mcids_by_page = collect_descendant_mcids_by_page(elem);
    let metadata = content_metadata_for(
        state,
        current_lang.clone(),
        expansion_text.clone(),
        elem.source_role.as_deref(),
    );
    let node_pages = semantic_node_pages(elem.page, &descendant_mcids_by_page);
    let node_id = TaggedNodeId(semantic_nodes.len());
    semantic_nodes.push(TaggedSemanticNode {
        id: node_id,
        parent: parent_node_id,
        struct_type: elem.struct_type.clone(),
        source_role: elem.source_role.clone(),
        page: elem.page,
        pages: node_pages.clone(),
        mcids_by_page: descendant_mcids_by_page.clone(),
        object_refs: Vec::new(),
        hints: metadata.hints.clone(),
        lang: metadata.lang.clone(),
        lang_origin: metadata.lang_origin,
        alt_text: image_alt_text.clone(),
        expansion_text: expansion_text.clone(),
        actual_text: actual_text.clone(),
        children: Vec::new(),
    });
    register_node_pages(page_contexts, node_id, &node_pages);
    if let Some(parent_node_id) = parent_node_id {
        semantic_nodes[parent_node_id.0].children.push(node_id);
    }
    if let Some(start_hint) = section_start_hint_for(elem) {
        push_page_scoped_reading_item(
            page_contexts,
            &node_pages,
            TaggedReadingItem::StructuralStart { hint: start_hint },
            true,
        );
    }

    if expansion_text.is_some()
        && image_alt_text.is_none()
        && actual_text.is_none()
        && descendant_mcids_by_page.is_empty()
    {
        summary.expansion_without_mcid += 1;
    }

    if let Some(alt_text) = image_alt_text {
        if descendant_mcids_by_page.is_empty() {
            summary.alt_text_without_mcid += 1;
            if expansion_text.is_some() {
                summary.expansion_without_mcid += 1;
            }
        } else {
            for (page, mcids) in &descendant_mcids_by_page {
                let page_ctx = page_contexts.entry(*page).or_default();
                for mcid in mcids {
                    page_ctx.add_alt_text_candidate(*mcid, &alt_text);
                }
            }
        }
    }

    if let Some(actual_text) = actual_text.as_ref() {
        if descendant_mcids_by_page.is_empty() {
            summary.actual_text_without_mcid += 1;
            if expansion_text.is_some() {
                summary.expansion_without_mcid += 1;
            }
        } else {
            for (page, mcids) in &descendant_mcids_by_page {
                let page_ctx = page_contexts.entry(*page).or_default();
                for mcid in mcids {
                    page_ctx.merge_metadata(*mcid, &metadata);
                }
                page_ctx.push_reading_item(TaggedReadingItem::ActualText {
                    text: actual_text.clone(),
                    mcids: mcids.clone(),
                    metadata: metadata.clone(),
                });
            }
        }
    }
    if actual_text.is_none() {
        for child in &elem.children {
            match child {
                StructChild::MarkedContentRef { mcid, page } => {
                    let page_ctx = page_contexts.entry(*page).or_default();
                    page_ctx.merge_metadata(*mcid, &metadata);
                    page_ctx.push_reading_item(TaggedReadingItem::Mcid {
                        mcid: *mcid,
                        metadata: metadata.clone(),
                    });
                }
                StructChild::StructElem(child_elem) => {
                    traverse_structure_element(
                        child_elem,
                        state,
                        current_lang.clone(),
                        Some(node_id),
                        page_contexts,
                        semantic_nodes,
                        summary,
                    );
                }
                StructChild::ObjectRef(object_num, generation) => {
                    semantic_nodes[node_id.0]
                        .object_refs
                        .push((*object_num, *generation));
                    summary.object_ref_count += 1;
                }
            }
        }
    }

    if let Some(end_hint) = section_end_hint_for(elem) {
        push_page_scoped_reading_item(
            page_contexts,
            &node_pages,
            TaggedReadingItem::StructuralEnd { hint: end_hint },
            false,
        );
    }

    unwind_traversal_state(elem, state, unwind_state);
    node_id
}

fn semantic_node_pages(elem_page: Option<u32>, mcids_by_page: &HashMap<u32, Vec<u32>>) -> Vec<u32> {
    let mut pages: Vec<u32> = mcids_by_page.keys().copied().collect();
    if let Some(page) = elem_page {
        pages.push(page);
    }
    pages.sort_unstable();
    pages.dedup();
    pages
}

fn register_node_pages(
    page_contexts: &mut HashMap<u32, TaggedPageContext>,
    node_id: TaggedNodeId,
    pages: &[u32],
) {
    for page in pages {
        page_contexts
            .entry(*page)
            .or_default()
            .register_semantic_node(node_id);
    }
}

fn push_page_scoped_reading_item(
    page_contexts: &mut HashMap<u32, TaggedPageContext>,
    pages: &[u32],
    item: TaggedReadingItem,
    first_page_only: bool,
) {
    let target_page = if first_page_only {
        pages.first().copied()
    } else {
        pages.last().copied()
    };

    if let Some(page) = target_page {
        page_contexts
            .entry(page)
            .or_default()
            .push_reading_item(item);
    }
}

fn unwind_traversal_state(
    elem: &StructElem,
    state: &mut TraversalState,
    unwind_state: TraversalUnwindState,
) {
    if matches!(elem.struct_type, StructType::LI) {
        state.active_list_item = unwind_state.previous_list_item;
    }

    if matches!(elem.struct_type, StructType::TH | StructType::TD) {
        state.active_cell = unwind_state.previous_cell;
    }

    if matches!(elem.struct_type, StructType::TR)
        && let Some(table) = state.table_stack.last_mut()
    {
        table.current_row = unwind_state.previous_row;
    }

    if unwind_state.pushed_table {
        let _ = state.table_stack.pop();
    }

    if unwind_state.pushed_list {
        let _ = state.list_stack.pop();
    }

    if unwind_state.pushed_link {
        let _ = state.link_stack.pop();
    }

    if unwind_state.pushed_block.is_some() {
        let _ = state.block_stack.pop();
    }
}

fn collect_descendant_mcids_by_page(elem: &StructElem) -> HashMap<u32, Vec<u32>> {
    let mut by_page = HashMap::new();
    collect_descendant_mcids_by_page_recursive(elem, &mut by_page);
    for mcids in by_page.values_mut() {
        let mut seen = HashSet::new();
        mcids.retain(|mcid| seen.insert(*mcid));
    }
    by_page
}

fn collect_descendant_mcids_by_page_recursive(
    elem: &StructElem,
    by_page: &mut HashMap<u32, Vec<u32>>,
) {
    for child in &elem.children {
        match child {
            StructChild::MarkedContentRef { mcid, page } => {
                by_page.entry(*page).or_default().push(*mcid);
            }
            StructChild::StructElem(child_elem) => {
                collect_descendant_mcids_by_page_recursive(child_elem, by_page);
            }
            StructChild::ObjectRef(_, _) => {}
        }
    }
}
