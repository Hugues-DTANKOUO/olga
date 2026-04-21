//! End-to-end integration tests: real documents → decode → StructureEngine → assertions.
//!
//! These tests use real document fixtures (created with python-docx, reportlab,
//! openpyxl) to validate the full pipeline from byte stream to DocumentNode tree.
//! They complement the unit tests (which use hand-built primitives) by exercising
//! the actual decoder output through the structure engine.

use olga::model::{DocumentNode, NodeKind};
use olga::structure::{StructureConfig, StructureEngine};

#[path = "e2e_structure/cross_format.rs"]
mod cross_format;
#[path = "e2e_structure/docx.rs"]
mod docx;
#[path = "e2e_structure/docx_stress.rs"]
mod docx_stress;
#[path = "e2e_structure/html.rs"]
mod html;
#[path = "e2e_structure/html_stress.rs"]
mod html_stress;
#[path = "e2e_structure/pdf.rs"]
mod pdf;
#[path = "e2e_structure/pdf_stress.rs"]
mod pdf_stress;
#[path = "e2e_structure/xlsx.rs"]
mod xlsx;
#[path = "e2e_structure/xlsx_stress.rs"]
mod xlsx_stress;

pub(crate) fn collect_nodes_by_kind<'a>(
    node: &'a DocumentNode,
    predicate: &dyn Fn(&NodeKind) -> bool,
) -> Vec<&'a DocumentNode> {
    let mut result = Vec::new();
    if predicate(&node.kind) {
        result.push(node);
    }
    for child in &node.children {
        result.extend(collect_nodes_by_kind(child, predicate));
    }
    result
}

pub(crate) fn count_nodes_by_kind(
    node: &DocumentNode,
    predicate: &dyn Fn(&NodeKind) -> bool,
) -> usize {
    collect_nodes_by_kind(node, predicate).len()
}

pub(crate) fn all_text_of(node: &DocumentNode) -> String {
    node.all_text()
}

pub(crate) fn build_engine() -> StructureEngine {
    StructureEngine::new(StructureConfig::default()).with_default_detectors()
}
