use std::collections::BTreeMap;

use pdf_oxide::PdfDocument;
use pdf_oxide::content::GraphicsStateStack;
use pdf_oxide::elements::PathContent;
use pdf_oxide::extractors::text::ArtifactType;
use pdf_oxide::geometry::{Point as PdfPoint, Rect as PdfRect};
use pdf_oxide::object::ObjectRef;

use crate::error::{Warning, WarningKind};

use super::marked_content::{MarkedContentContext, describe_artifact_kind};
use super::types::RawPath;

pub(super) mod classify;
mod geometry;
pub(super) mod scan;

const LINE_TOLERANCE_PT: f32 = 2.0;
const PATH_ARTIFACT_MATCH_TOLERANCE_PT: f32 = 1.0;

#[derive(Debug, Clone)]
pub(super) struct ArtifactPathCandidate {
    pub(super) path: RawPath,
    pub(super) artifact_type: Option<ArtifactType>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct ArtifactPathBuilder {
    pub(super) commands: Vec<ArtifactPathCommand>,
    pub(super) has_complex_segments: bool,
}

#[derive(Debug, Clone)]
pub(super) enum ArtifactPathCommand {
    MoveTo(PdfPoint),
    LineTo(PdfPoint),
    Rectangle(PdfRect),
    ClosePath,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PaintMode {
    pub(super) stroke: bool,
    pub(super) fill: bool,
}

impl ArtifactPathBuilder {
    pub(super) fn clear(&mut self) {
        self.commands.clear();
        self.has_complex_segments = false;
    }

    pub(super) fn has_path(&self) -> bool {
        self.has_complex_segments || !self.commands.is_empty()
    }
}

#[derive(Debug)]
pub(super) struct PathArtifactScanState<'a> {
    pub(super) xobject_stack: Vec<ObjectRef>,
    pub(super) marked_content_stack: Vec<MarkedContentContext>,
    pub(super) graphics_state_stack: GraphicsStateStack,
    pub(super) current_path: ArtifactPathBuilder,
    pub(super) artifact_paths: Vec<ArtifactPathCandidate>,
    pub(super) unsupported_artifact_counts: BTreeMap<&'static str, usize>,
    warnings: &'a mut Vec<Warning>,
    page_num: u32,
}

impl<'a> PathArtifactScanState<'a> {
    pub(super) fn new(page_num: u32, warnings: &'a mut Vec<Warning>) -> Self {
        Self {
            xobject_stack: Vec::new(),
            marked_content_stack: Vec::new(),
            graphics_state_stack: GraphicsStateStack::new(),
            current_path: ArtifactPathBuilder::default(),
            artifact_paths: Vec::new(),
            unsupported_artifact_counts: BTreeMap::new(),
            warnings,
            page_num,
        }
    }

    pub(super) fn warn(&mut self, message: String) {
        self.warnings.push(Warning {
            kind: WarningKind::PartialExtraction,
            message,
            page: Some(self.page_num),
        });
    }
}

pub(super) fn scan_page_artifact_paths(
    doc: &mut PdfDocument,
    page_idx: usize,
    warnings: &mut Vec<Warning>,
    cached_resources: Option<&pdf_oxide::object::Object>,
) -> Option<Vec<ArtifactPathCandidate>> {
    scan::scan_page_artifact_paths(doc, page_idx, warnings, cached_resources)
}

pub(super) fn filter_artifact_paths(
    paths: Vec<RawPath>,
    artifact_candidates: Option<&[ArtifactPathCandidate]>,
    page: u32,
    warnings: &mut Vec<Warning>,
) -> Vec<RawPath> {
    classify::filter_artifact_paths(paths, artifact_candidates, page, warnings)
}

pub(super) fn convert_paths(
    paths: &[PathContent],
    page: u32,
    warnings: &mut Vec<Warning>,
) -> Vec<RawPath> {
    classify::convert_paths(paths, page, warnings)
}
