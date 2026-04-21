//! Stress tests for the Structure Engine (Brick 5).
//!
//! These tests use programmatically built DOCX, HTML, and XLSX fixtures to
//! exercise edge cases that the corpus fixtures don't cover. Each test targets
//! a specific structural scenario that could break the assembler, classify(),
//! cross-page continuity, or detector logic.

mod support;

use olga::model::{DocumentNode, NodeKind};
use olga::structure::{StructureConfig, StructureEngine};

#[path = "e2e_structure_stress/cross_format.rs"]
mod cross_format;
#[path = "e2e_structure_stress/docx.rs"]
mod docx;
#[path = "e2e_structure_stress/html.rs"]
mod html;
#[path = "e2e_structure_stress/layout.rs"]
mod layout;
#[path = "e2e_structure_stress/xlsx.rs"]
mod xlsx;

pub(crate) fn build_engine() -> StructureEngine {
    StructureEngine::new(StructureConfig::default()).with_default_detectors()
}

pub(crate) fn collect_nodes<'a>(
    node: &'a DocumentNode,
    predicate: &dyn Fn(&NodeKind) -> bool,
) -> Vec<&'a DocumentNode> {
    let mut result = Vec::new();
    if predicate(&node.kind) {
        result.push(node);
    }
    for child in &node.children {
        result.extend(collect_nodes(child, predicate));
    }
    result
}

pub(crate) fn count_nodes(node: &DocumentNode, predicate: &dyn Fn(&NodeKind) -> bool) -> usize {
    collect_nodes(node, predicate).len()
}

pub(crate) fn run_structure(data: olga::traits::DecodeResult) -> olga::structure::StructureResult {
    build_engine().structure(data)
}
