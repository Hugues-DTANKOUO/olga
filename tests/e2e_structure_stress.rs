//! Stress tests for the Structure Engine (Brick 5).
//!
//! These tests use programmatically built DOCX, HTML, and XLSX fixtures to
//! exercise edge cases that the corpus fixtures don't cover. Each test targets
//! a specific structural scenario that could break the assembler, classify(),
//! cross-page continuity, or detector logic.
//!
//! Per v0.1.2 [features] block : this test binary is gated on the
//! union of the 4 per-format features. When none is enabled, the
//! binary compiles to an empty shell.

#![cfg(any(feature = "docx", feature = "html", feature = "pdf", feature = "xlsx"))]

mod support;

use olga::model::{DocumentNode, NodeKind};
use olga::structure::{StructureConfig, StructureEngine};

#[cfg(all(feature = "docx", feature = "html", feature = "xlsx"))]
#[path = "e2e_structure_stress/cross_format.rs"]
mod cross_format;
#[cfg(feature = "docx")]
#[path = "e2e_structure_stress/docx.rs"]
mod docx;
#[cfg(feature = "html")]
#[path = "e2e_structure_stress/html.rs"]
mod html;
#[cfg(feature = "pdf")]
#[path = "e2e_structure_stress/layout.rs"]
mod layout;
#[cfg(feature = "xlsx")]
#[path = "e2e_structure_stress/xlsx.rs"]
mod xlsx;

// Helpers below are #[allow(dead_code)] because some are only
// referenced by a subset of the gated per-format sub-modules ; when
// a feature combo enables only some sub-modules, the unused helpers
// would otherwise trigger dead_code warnings (which fail under
// the workspace's clippy-strict CI gate).
#[allow(dead_code)]
pub(crate) fn build_engine() -> StructureEngine {
    StructureEngine::new(StructureConfig::default()).with_default_detectors()
}

#[allow(dead_code)]
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

#[allow(dead_code)]
pub(crate) fn count_nodes(node: &DocumentNode, predicate: &dyn Fn(&NodeKind) -> bool) -> usize {
    collect_nodes(node, predicate).len()
}

#[allow(dead_code)]
pub(crate) fn run_structure(data: olga::traits::DecodeResult) -> olga::structure::StructureResult {
    build_engine().structure(data)
}
