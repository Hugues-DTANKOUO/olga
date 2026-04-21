use super::visual::heading_text_looks_valid;
use super::*;
use crate::model::{HintSource, PrimitiveKind};

impl Assembler {
    pub(super) fn structure_source_for(prim: &Primitive) -> StructureSource {
        if let Some(hint) = prim.hints.first() {
            match &hint.source {
                HintSource::FormatDerived => StructureSource::HintAssembled,
                HintSource::HeuristicInferred { detector } => StructureSource::HeuristicDetected {
                    detector: detector.clone(),
                    confidence: hint.confidence,
                },
            }
        } else {
            StructureSource::HintAssembled
        }
    }

    pub(super) fn text_of(prim: &Primitive) -> String {
        match &prim.kind {
            PrimitiveKind::Text { content, .. } => content.clone(),
            PrimitiveKind::Image { alt_text, .. } => alt_text.as_deref().unwrap_or("").to_string(),
            _ => String::new(),
        }
    }

    pub(super) fn font_size_of(prim: &Primitive) -> Option<f32> {
        match &prim.kind {
            PrimitiveKind::Text { font_size, .. } => Some(*font_size),
            _ => None,
        }
    }

    pub(super) fn should_demote_heading(&self, prim: &Primitive, _confidence: f32) -> bool {
        let text_invalid = prim
            .text_content()
            .is_some_and(|t| !heading_text_looks_valid(t));

        if text_invalid {
            return true;
        }

        let total_primitives: u32 = self.font_size_counts.values().sum();
        if total_primitives < 20 {
            return false;
        }

        Self::font_size_of(prim)
            .zip(self.body_font_size())
            .is_some_and(|(prim_size, body_size)| {
                prim_size > 3.0 && body_size > 3.0 && (prim_size / body_size - 1.0).abs() < 0.05
            })
    }

    pub(super) fn track_font_size(&mut self, prim: &Primitive) {
        if let Some(size) = Self::font_size_of(prim) {
            let key = (size * 10.0).round() as u32;
            *self.font_size_counts.entry(key).or_insert(0) += 1;
        }
    }

    pub(super) fn body_font_size(&self) -> Option<f32> {
        self.font_size_counts
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(key, _)| *key as f32 / 10.0)
    }

    pub(super) fn merged_bbox(&self, nodes: &[DocumentNode]) -> BoundingBox {
        self.merged_bbox_opt(nodes)
            .unwrap_or(BoundingBox::new(0.0, 0.0, 1.0, 1.0))
    }

    pub(super) fn merged_bbox_opt(&self, nodes: &[DocumentNode]) -> Option<BoundingBox> {
        nodes.iter().map(|n| n.bbox).reduce(|a, b| a.merge(&b))
    }
}
