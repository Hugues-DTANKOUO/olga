pub(super) fn provenance_string(source: &crate::model::provenance::StructureSource) -> String {
    use crate::model::provenance::StructureSource;
    match source {
        StructureSource::HintAssembled => "format-derived".to_string(),
        StructureSource::HeuristicDetected {
            detector,
            confidence: _,
        } => format!("heuristic:{}", detector),
    }
}

// ---------------------------------------------------------------------------
// Rounding helpers (avoid noisy floating-point output)
// ---------------------------------------------------------------------------

pub(super) fn round2(v: f32) -> f32 {
    (v * 100.0).round() / 100.0
}

pub(super) fn round4(v: f32) -> f32 {
    (v * 10000.0).round() / 10000.0
}
