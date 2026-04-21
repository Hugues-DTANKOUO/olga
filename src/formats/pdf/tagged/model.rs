use std::collections::HashMap;

use pdf_oxide::extractors::text::ArtifactType;
use pdf_oxide::structure::StructType;
use unicode_normalization::UnicodeNormalization;

use crate::model::{GeometryProvenance, SemanticHint, ValueOrigin};

use super::super::types::TextSpan;

#[derive(Debug, Clone, Default)]
pub(crate) struct TaggedPdfContext {
    pub(super) page_contexts: HashMap<u32, TaggedPageContext>,
    pub(super) semantic_nodes: Vec<TaggedSemanticNode>,
    pub(super) use_structure_reading_order: bool,
}

impl TaggedPdfContext {
    pub fn page_context(&self, page: u32) -> Option<&TaggedPageContext> {
        self.page_contexts.get(&page)
    }

    pub fn use_structure_reading_order(&self) -> bool {
        self.use_structure_reading_order
    }

    pub(crate) fn semantic_node_count(&self) -> usize {
        self.semantic_nodes.len()
    }

    pub(super) fn semantic_graph_stats(&self) -> (usize, usize) {
        let mut node_count = 0usize;
        let mut object_ref_count = 0usize;
        let mut page_local_node_count = 0usize;
        let mut page_binding_count = 0usize;
        let mut mcid_binding_count = 0usize;
        let mut child_edge_count = 0usize;
        let mut parented_node_count = 0usize;
        let mut explicit_page_node_count = 0usize;
        let mut hinted_node_count = 0usize;
        let mut language_annotated_node_count = 0usize;
        let mut alt_text_node_count = 0usize;
        let mut expansion_node_count = 0usize;
        let mut actual_text_node_count = 0usize;
        let mut custom_role_node_count = 0usize;
        let mut section_like_node_count = 0usize;

        for node in &self.semantic_nodes {
            node_count += 1;
            object_ref_count += node.object_refs.len();
            page_binding_count += node.pages.len();
            mcid_binding_count += node.mcids_by_page.values().map(Vec::len).sum::<usize>();
            child_edge_count += node.children.len();
            if node.parent.is_some() {
                parented_node_count += 1;
            }
            if node.page.is_some() {
                explicit_page_node_count += 1;
            }
            if !node.hints.is_empty() {
                hinted_node_count += 1;
            }
            if node.lang.is_some() || node.lang_origin.is_some() {
                language_annotated_node_count += 1;
            }
            if node.alt_text.is_some() {
                alt_text_node_count += 1;
            }
            if node.expansion_text.is_some() {
                expansion_node_count += 1;
            }
            if node.actual_text.is_some() {
                actual_text_node_count += 1;
            }
            if node.source_role.is_some() {
                custom_role_node_count += 1;
            }
            if matches!(
                node.struct_type,
                StructType::Part | StructType::Art | StructType::Sect | StructType::Div
            ) {
                section_like_node_count += 1;
            }
            debug_assert_eq!(node.id.0, node_count - 1);
        }

        for page_ctx in self.page_contexts.values() {
            page_local_node_count += page_ctx.semantic_node_ids.len();
        }

        let _ = (
            page_local_node_count,
            page_binding_count,
            mcid_binding_count,
            child_edge_count,
            parented_node_count,
            explicit_page_node_count,
            hinted_node_count,
            language_annotated_node_count,
            alt_text_node_count,
            expansion_node_count,
            actual_text_node_count,
            custom_role_node_count,
            section_like_node_count,
        );

        (node_count, object_ref_count)
    }

    pub(super) fn semantic_node_count_for_debug(&self) -> usize {
        self.semantic_node_count()
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TaggedPageContext {
    pub(super) reading_items: Vec<TaggedReadingItem>,
    pub(super) metadata_by_mcid: HashMap<u32, TaggedContentMetadata>,
    pub(super) alt_text_by_mcid: HashMap<u32, Vec<String>>,
    pub(super) semantic_node_ids: Vec<TaggedNodeId>,
}

impl TaggedPageContext {
    pub fn has_reading_items(&self) -> bool {
        !self.reading_items.is_empty()
    }

    pub fn references_mcid(&self, mcid: u32) -> bool {
        self.metadata_by_mcid.contains_key(&mcid)
    }

    pub(super) fn push_reading_item(&mut self, item: TaggedReadingItem) {
        self.reading_items.push(item);
    }

    pub(super) fn merge_metadata(&mut self, mcid: u32, metadata: &TaggedContentMetadata) {
        self.metadata_by_mcid
            .entry(mcid)
            .or_default()
            .merge_from(metadata);
    }

    pub(super) fn add_alt_text_candidate(&mut self, mcid: u32, alt_text: &str) {
        let candidates = self.alt_text_by_mcid.entry(mcid).or_default();
        if !candidates.iter().any(|candidate| candidate == alt_text) {
            candidates.push(alt_text.to_string());
        }
    }

    pub(super) fn metadata_for_mcid(&self, mcid: Option<u32>) -> TaggedContentMetadata {
        mcid.and_then(|mcid| self.metadata_by_mcid.get(&mcid).cloned())
            .unwrap_or_default()
    }

    pub(super) fn has_alt_text_candidates(&self) -> bool {
        !self.alt_text_by_mcid.is_empty()
    }

    pub(super) fn resolve_alt_text_for_mcid(&self, mcid: Option<u32>) -> ImageAltTextResolution {
        let Some(mcid) = mcid else {
            return ImageAltTextResolution::None;
        };

        match self.alt_text_by_mcid.get(&mcid) {
            None => ImageAltTextResolution::None,
            Some(candidates) if candidates.len() == 1 => {
                ImageAltTextResolution::Unique(candidates[0].clone())
            }
            Some(candidates) => ImageAltTextResolution::Ambiguous(candidates.clone()),
        }
    }

    pub(super) fn register_semantic_node(&mut self, node_id: TaggedNodeId) {
        if !self.semantic_node_ids.contains(&node_id) {
            self.semantic_node_ids.push(node_id);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct TaggedNodeId(pub(super) usize);

#[derive(Debug, Clone)]
pub(crate) struct TaggedSemanticNode {
    pub(super) id: TaggedNodeId,
    pub(super) parent: Option<TaggedNodeId>,
    pub(super) struct_type: StructType,
    pub(super) source_role: Option<String>,
    pub(super) page: Option<u32>,
    pub(super) pages: Vec<u32>,
    pub(super) mcids_by_page: HashMap<u32, Vec<u32>>,
    pub(super) object_refs: Vec<(u32, u16)>,
    pub(super) hints: Vec<SemanticHint>,
    pub(super) lang: Option<String>,
    pub(super) lang_origin: Option<ValueOrigin>,
    pub(super) alt_text: Option<String>,
    pub(super) expansion_text: Option<String>,
    pub(super) actual_text: Option<String>,
    pub(super) children: Vec<TaggedNodeId>,
}

#[derive(Debug, Clone)]
pub(crate) struct TaggedTextSpan {
    pub span: TextSpan,
    pub hints: Vec<SemanticHint>,
    pub lang: Option<String>,
    pub lang_origin: Option<ValueOrigin>,
    pub geometry_provenance: GeometryProvenance,
}

impl TaggedTextSpan {
    pub(crate) fn plain(span: TextSpan) -> Self {
        Self {
            span,
            hints: Vec::new(),
            lang: None,
            lang_origin: None,
            geometry_provenance: GeometryProvenance::observed_physical(),
        }
    }

    pub(super) fn with_metadata(
        span: TextSpan,
        metadata: TaggedContentMetadata,
        geometry_provenance: GeometryProvenance,
    ) -> Self {
        Self {
            span,
            hints: metadata.hints,
            lang: metadata.lang,
            lang_origin: metadata.lang_origin,
            geometry_provenance,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TaggedContentMetadata {
    pub(super) hints: Vec<SemanticHint>,
    pub(super) lang: Option<String>,
    pub(super) lang_origin: Option<ValueOrigin>,
}

impl TaggedContentMetadata {
    pub(super) fn add_hint(&mut self, hint: SemanticHint) {
        if !self.hints.contains(&hint) {
            self.hints.push(hint);
        }
    }

    pub(super) fn merge_from(&mut self, other: &TaggedContentMetadata) {
        for hint in &other.hints {
            self.add_hint(hint.clone());
        }
        if self.lang.is_none() {
            self.lang = other.lang.clone();
            self.lang_origin = other.lang_origin;
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum TaggedReadingItem {
    StructuralStart {
        hint: SemanticHint,
    },
    StructuralEnd {
        hint: SemanticHint,
    },
    Mcid {
        mcid: u32,
        metadata: TaggedContentMetadata,
    },
    ActualText {
        text: String,
        mcids: Vec<u32>,
        metadata: TaggedContentMetadata,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaggedImageAppearance {
    pub mcid: Option<u32>,
    pub is_artifact: bool,
    pub artifact_type: Option<ArtifactType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ImageAltTextResolution {
    None,
    Unique(String),
    Ambiguous(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ImageAltBinding {
    Resolved(Vec<ImageAltTextResolution>),
    CountMismatch {
        appearance_count: usize,
        image_count: usize,
    },
    Unavailable,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TableState {
    pub(super) next_row: u32,
    pub(super) current_row: Option<RowState>,
}

impl TableState {
    pub(super) fn new() -> Self {
        Self {
            next_row: 0,
            current_row: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RowState {
    pub(super) row: u32,
    pub(super) next_col: u32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TableCellContext {
    pub(super) row: u32,
    pub(super) col: u32,
    pub(super) rowspan: u32,
    pub(super) colspan: u32,
    pub(super) is_header: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum BlockSemantic {
    Heading(u8),
    Paragraph,
    BlockQuote,
    CodeBlock,
    Sidebar,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ListContext {
    pub(super) depth: u8,
    pub(super) ordered: bool,
    pub(super) ordered_observed: bool,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TraversalState {
    pub(super) block_stack: Vec<BlockSemantic>,
    pub(super) table_stack: Vec<TableState>,
    pub(super) list_stack: Vec<ListContext>,
    pub(super) link_stack: Vec<SemanticHint>,
    pub(super) active_cell: Option<TableCellContext>,
    pub(super) active_list_item: Option<ListContext>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TraversalUnwindState {
    pub(super) pushed_block: Option<BlockSemantic>,
    pub(super) pushed_list: bool,
    pub(super) pushed_link: bool,
    pub(super) previous_list_item: Option<ListContext>,
    pub(super) pushed_table: bool,
    pub(super) previous_row: Option<RowState>,
    pub(super) previous_cell: Option<TableCellContext>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct TaggedTextApplicationStats {
    pub fallback_span_count: usize,
    pub unmatched_structure_item_count: usize,
    pub actual_text_replacement_count: usize,
    pub unapplied_structural_hint_count: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct TaggedTextApplicationResult {
    pub spans: Vec<TaggedTextSpan>,
    pub stats: TaggedTextApplicationStats,
}

pub(crate) fn normalize_text_value(value: Option<&str>) -> Option<String> {
    let normalized = value?.nfc().collect::<String>();
    let trimmed = normalized.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}
