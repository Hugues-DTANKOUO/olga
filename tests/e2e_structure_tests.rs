//! End-to-end integration tests: real documents → decode → StructureEngine → assertions.
//!
//! These tests use real document fixtures (created with python-docx, reportlab,
//! openpyxl) to validate the full pipeline from byte stream to DocumentNode tree.
//! They complement the unit tests (which use hand-built primitives) by exercising
//! the actual decoder output through the structure engine.
//!
//! Per v0.1.2 [features] block : this test binary is gated on the
//! union of the 4 per-format features. When none is enabled, the
//! binary compiles to an empty shell (zero tests, no helpers). Each
//! per-format sub-module below carries its own feature gate so that
//! enabling a single feature (e.g. `--features html`) keeps only
//! the matching sub-module + the shared helpers.

#![cfg(any(feature = "docx", feature = "html", feature = "pdf", feature = "xlsx"))]

use olga::model::{DocumentNode, NodeKind};
use olga::structure::{StructureConfig, StructureEngine};

// Per-format sub-modules are gated on their respective cargo
// features per v0.1.2 [features] block. The `cross_format` sub-
// module exercises multiple decoders simultaneously and is gated
// on the union of all four features.
#[cfg(all(feature = "docx", feature = "html", feature = "pdf", feature = "xlsx"))]
#[path = "e2e_structure/cross_format.rs"]
mod cross_format;
#[cfg(feature = "docx")]
#[path = "e2e_structure/docx.rs"]
mod docx;
#[cfg(feature = "docx")]
#[path = "e2e_structure/docx_stress.rs"]
mod docx_stress;
#[cfg(feature = "html")]
#[path = "e2e_structure/html.rs"]
mod html;
#[cfg(feature = "html")]
#[path = "e2e_structure/html_stress.rs"]
mod html_stress;
#[cfg(feature = "pdf")]
#[path = "e2e_structure/pdf.rs"]
mod pdf;
#[cfg(feature = "pdf")]
#[path = "e2e_structure/pdf_stress.rs"]
mod pdf_stress;
#[cfg(feature = "xlsx")]
#[path = "e2e_structure/xlsx.rs"]
mod xlsx;
#[cfg(feature = "xlsx")]
#[path = "e2e_structure/xlsx_stress.rs"]
mod xlsx_stress;

// Helpers below are #[allow(dead_code)] because some are only
// referenced by a subset of the gated per-format sub-modules ; when
// a feature combo enables only some sub-modules, the unused helpers
// would otherwise trigger dead_code warnings (which fail under
// the workspace's clippy-strict CI gate).
#[allow(dead_code)]
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

#[allow(dead_code)]
pub(crate) fn count_nodes_by_kind(
    node: &DocumentNode,
    predicate: &dyn Fn(&NodeKind) -> bool,
) -> usize {
    collect_nodes_by_kind(node, predicate).len()
}

#[allow(dead_code)]
pub(crate) fn all_text_of(node: &DocumentNode) -> String {
    node.all_text()
}

#[allow(dead_code)]
pub(crate) fn build_engine() -> StructureEngine {
    StructureEngine::new(StructureConfig::default()).with_default_detectors()
}
