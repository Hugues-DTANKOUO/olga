use super::*;

#[test]
fn decode_warnings_propagated_to_result() {
    let warnings = vec![Warning {
        kind: crate::error::WarningKind::UnsupportedElement,
        message: "Test warning".to_string(),
        page: Some(0),
    }];
    let prims = vec![hinted(text_prim(0, 0.1, "Text", 0), HintKind::Paragraph)];
    let engine = StructureEngine::new(StructureConfig::default());
    let result = engine.structure(decode_result_with_warnings(prims, warnings));

    assert!(result.warnings.iter().any(|w| w.message == "Test warning"));
}

#[test]
fn metadata_passed_through() {
    let mut meta = test_metadata();
    meta.title = Some("Test Document".to_string());
    meta.file_size = 42;

    let dr = DecodeResult {
        metadata: meta,
        primitives: vec![],
        warnings: vec![],
        images: vec![],
    };
    let engine = StructureEngine::new(StructureConfig::default());
    let result = engine.structure(dr);

    assert_eq!(result.metadata.title.as_deref(), Some("Test Document"));
    assert_eq!(result.metadata.file_size, 42);
}
