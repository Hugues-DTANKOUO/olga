use super::*;

#[test]
fn low_threshold_forces_detection_on_hinted_pages() {
    let prims = vec![hinted(
        text_prim(0, 0.1, "Hinted text", 0),
        HintKind::Paragraph,
    )];
    let config = StructureConfig {
        hint_trust_threshold: 0.0,
        ..StructureConfig::default()
    };

    let engine = StructureEngine::new(config).with_default_detectors();
    let result = engine.structure(decode_result(prims));

    assert!(result.heuristic_pages.is_empty());
}

#[test]
fn threshold_above_one_forces_detection_on_all_pages() {
    let prims = vec![hinted(
        text_prim(0, 0.1, "Hinted text", 0),
        HintKind::Paragraph,
    )];
    let config = StructureConfig {
        hint_trust_threshold: 1.01,
        ..StructureConfig::default()
    };

    let engine = StructureEngine::new(config).with_default_detectors();
    let result = engine.structure(decode_result(prims));

    assert_eq!(result.heuristic_pages, vec![0]);
}

#[test]
fn mixed_hinted_and_unhinted_pages() {
    let prims = vec![
        hinted(
            text_prim(0, 0.1, "Hinted paragraph", 0),
            HintKind::Paragraph,
        ),
        text_prim(1, 0.1, "Bare text on page 1", 1),
    ];
    let engine = StructureEngine::new(StructureConfig::default()).with_default_detectors();
    let result = engine.structure(decode_result(prims));

    assert_eq!(result.heuristic_pages, vec![1]);
}
