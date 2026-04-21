//! Tagged PDF helpers: structure-tree-derived reading order and semantic hints.

#[path = "tagged/load.rs"]
mod load;
#[path = "tagged/model.rs"]
mod model;
#[path = "tagged/semantics.rs"]
mod semantics;
#[path = "tagged/text_apply.rs"]
mod text_apply;
#[path = "tagged/traversal.rs"]
mod traversal;

use semantics::add_hint_if_missing;

pub(crate) use load::{bind_alt_text_to_image_appearances, load_tagged_pdf_context};
pub(crate) use model::{
    BlockSemantic, ImageAltBinding, ImageAltTextResolution, ListContext, RowState,
    TableCellContext, TableState, TaggedContentMetadata, TaggedImageAppearance, TaggedNodeId,
    TaggedPageContext, TaggedPdfContext, TaggedReadingItem, TaggedSemanticNode,
    TaggedTextApplicationResult, TaggedTextApplicationStats, TaggedTextSpan, TraversalState,
    TraversalUnwindState, normalize_text_value,
};
pub(crate) use text_apply::apply_tagged_structure_to_text_spans;

#[cfg(test)]
#[path = "tagged/tests.rs"]
mod tests;
