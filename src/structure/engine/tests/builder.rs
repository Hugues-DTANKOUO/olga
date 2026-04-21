use super::*;
use crate::model::NodeKind;

#[test]
fn with_detector_adds_custom_detector() {
    use crate::structure::detectors::{DetectedHint, DetectionContext, StructureDetector};

    struct TestDetector;
    impl StructureDetector for TestDetector {
        fn name(&self) -> &str {
            "test-detector"
        }
        fn detect(
            &self,
            page: &[crate::model::Primitive],
            _context: &DetectionContext,
        ) -> Vec<DetectedHint> {
            let mut hints = Vec::new();
            for (i, prim) in page.iter().enumerate() {
                if matches!(prim.kind, PrimitiveKind::Text { .. }) && prim.hints.is_empty() {
                    hints.push(DetectedHint {
                        primitive_index: i,
                        hint: SemanticHint::from_heuristic(
                            HintKind::CodeBlock,
                            0.9,
                            "test-detector",
                        ),
                    });
                    break;
                }
            }
            hints
        }
    }

    let prims = vec![text_prim(0, 0.1, "some_code_here", 0)];
    let engine =
        StructureEngine::new(StructureConfig::default()).with_detector(Box::new(TestDetector));
    let result = engine.structure(decode_result(prims));

    assert_eq!(result.root.children.len(), 1);
    match &result.root.children[0].kind {
        NodeKind::CodeBlock { text, .. } => assert_eq!(text, "some_code_here"),
        other => panic!("expected CodeBlock, got {:?}", other),
    }
}

#[test]
fn with_default_detectors_registers_paragraph_and_heading() {
    let engine = StructureEngine::new(StructureConfig::default()).with_default_detectors();
    let prims = vec![text_prim(0, 0.1, "Some text content", 0)];
    let result = engine.structure(decode_result(prims));

    assert_eq!(result.root.children.len(), 1);
    assert!(matches!(
        result.root.children[0].kind,
        NodeKind::Paragraph { .. }
    ));
}
