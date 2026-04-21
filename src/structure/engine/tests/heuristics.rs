use super::*;
use crate::model::NodeKind;
use crate::structure::detectors::HeadingDetector;

#[test]
fn heading_detected_by_font_size_heuristic() {
    let mut prims: Vec<Primitive> = (0..100)
        .map(|i| {
            text_prim_sized(
                0,
                0.01 * i as f32,
                12.0,
                false,
                "Body text content here.",
                i,
            )
        })
        .collect();
    prims.insert(
        0,
        text_prim_sized(0, 0.0, 24.0, true, "Major Heading Here", 200),
    );

    let engine = StructureEngine::new(StructureConfig::default())
        .with_detector(Box::new(HeadingDetector::default()));
    let result = engine.structure(decode_result(prims));

    let headings: Vec<_> = result
        .root
        .children
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::Heading { .. }))
        .collect();
    assert!(
        !headings.is_empty(),
        "Expected at least one heading to be detected by font-size heuristic"
    );

    assert_eq!(result.heuristic_pages, vec![0]);
}
