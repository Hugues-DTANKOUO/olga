use olga::model::*;

#[test]
fn serialize_roundtrip_bounding_box() {
    let bb = BoundingBox::new(0.1, 0.2, 0.3, 0.4);
    let json = serde_json::to_string(&bb).unwrap();
    let bb2: BoundingBox = serde_json::from_str(&json).unwrap();
    assert_eq!(bb, bb2);
}

#[test]
fn serialize_roundtrip_primitive_text() {
    let prim = Primitive::observed(
        PrimitiveKind::Text {
            content: "Test text".to_string(),
            font_size: 12.0,
            font_name: "Helvetica".to_string(),
            is_bold: true,
            is_italic: false,
            color: Some(Color::rgb(255, 0, 0)),
            char_positions: None,
            text_direction: TextDirection::RightToLeft,
            text_direction_origin: ValueOrigin::Observed,
            lang: None,
            lang_origin: None,
        },
        BoundingBox::new(0.05, 0.1, 0.9, 0.03),
        5,
        42,
    )
    .with_hint(SemanticHint::from_format(HintKind::Heading { level: 2 }));

    let json = serde_json::to_string_pretty(&prim).unwrap();
    let prim2: Primitive = serde_json::from_str(&json).unwrap();
    assert_eq!(prim2.page, 5);
    assert_eq!(prim2.source_order, 42);
    assert_eq!(prim2.hints.len(), 1);
    assert_eq!(prim2.text_content(), Some("Test text"));
}

#[test]
fn serialize_roundtrip_primitive_image() {
    let prim = Primitive::observed(
        PrimitiveKind::Image {
            format: ImageFormat::Png,
            data: vec![0x89, 0x50, 0x4E, 0x47],
            alt_text: Some("A test image".to_string()),
        },
        BoundingBox::new(0.1, 0.2, 0.5, 0.3),
        1,
        10,
    );

    let json = serde_json::to_string(&prim).unwrap();
    let prim2: Primitive = serde_json::from_str(&json).unwrap();
    match &prim2.kind {
        PrimitiveKind::Image { data, alt_text, .. } => {
            assert_eq!(data, &vec![0x89, 0x50, 0x4E, 0x47]);
            assert_eq!(alt_text.as_deref(), Some("A test image"));
        }
        _ => panic!("Expected Image primitive"),
    }
}

#[test]
fn serialize_roundtrip_document_node() {
    let node = DocumentNode::new(
        NodeKind::Heading {
            level: 1,
            text: "Introduction".to_string(),
        },
        BoundingBox::new(0.05, 0.02, 0.9, 0.04),
        1.0,
        StructureSource::HintAssembled,
        0..1,
    )
    .with_metadata("style", "Heading1");

    let json = serde_json::to_string_pretty(&node).unwrap();
    let node2: DocumentNode = serde_json::from_str(&json).unwrap();
    assert_eq!(node2.text(), Some("Introduction"));
    assert_eq!(node2.metadata.get("style"), Some(&"Heading1".to_string()));
}

#[test]
fn serialize_roundtrip_hint_kinds() {
    let hints = vec![
        HintKind::Heading { level: 3 },
        HintKind::Paragraph,
        HintKind::ListItem {
            depth: 2,
            ordered: true,
            list_group: None,
        },
        HintKind::TableCell {
            row: 1,
            col: 2,
            rowspan: 1,
            colspan: 3,
        },
        HintKind::ContainedInTableCell {
            row: 1,
            col: 2,
            depth: 0,
        },
        HintKind::FlattenedFromTableCell {
            row: 0,
            col: 1,
            depth: 2,
        },
        HintKind::TableHeader { col: 0 },
        HintKind::BlockQuote,
        HintKind::CodeBlock,
        HintKind::PageHeader,
        HintKind::PageFooter,
        HintKind::Footnote {
            id: "fn1".to_string(),
        },
        HintKind::Sidebar,
        HintKind::Link {
            url: "https://example.com".to_string(),
        },
        HintKind::ExpandedText {
            text: "Portable Document Format".to_string(),
        },
        HintKind::Emphasis,
        HintKind::Strong,
        HintKind::SectionStart {
            title: Some("Chapter 1".to_string()),
        },
        HintKind::SectionEnd,
        HintKind::DocumentTitle,
    ];

    for hint in hints {
        let json = serde_json::to_string(&hint).unwrap();
        let hint2: HintKind = serde_json::from_str(&json).unwrap();
        assert_eq!(hint, hint2);
    }
}

#[test]
fn serialize_roundtrip_page_dimensions() {
    let pd = PageDimensions::new(
        RawBox::new(0.0, 0.0, 612.0, 792.0),
        Some(RawBox::new(20.0, 20.0, 572.0, 752.0)),
        90,
    );
    let json = serde_json::to_string(&pd).unwrap();
    let pd2: PageDimensions = serde_json::from_str(&json).unwrap();
    assert_eq!(pd, pd2);
}

#[test]
fn serialize_roundtrip_document_metadata() {
    let meta = DocumentMetadata {
        format: DocumentFormat::Docx,
        page_count: 42,
        page_count_provenance: PaginationProvenance::estimated(
            PaginationBasis::ApplicationMetadata,
        ),
        page_dimensions: vec![PageDimensions::new(
            RawBox::new(0.0, 0.0, 612.0, 792.0),
            None,
            0,
        )],
        encrypted: false,
        text_extraction_allowed: true,
        file_size: 500_000,
        title: Some("My Document".to_string()),
    };
    let json = serde_json::to_string_pretty(&meta).unwrap();
    let meta2: DocumentMetadata = serde_json::from_str(&json).unwrap();
    assert_eq!(meta2.format, DocumentFormat::Docx);
    assert_eq!(meta2.page_count, 42);
    assert_eq!(meta2.title.as_deref(), Some("My Document"));
}

#[test]
fn serde_roundtrip_primitive_text() {
    let prim = Primitive::observed(
        PrimitiveKind::Text {
            content: "Hello world".to_string(),
            font_size: 12.0,
            font_name: "Arial".to_string(),
            is_bold: true,
            is_italic: false,
            color: Some(Color::rgb(255, 0, 0)),
            char_positions: None,
            text_direction: TextDirection::LeftToRight,
            text_direction_origin: ValueOrigin::Observed,
            lang: Some("en-US".to_string()),
            lang_origin: Some(ValueOrigin::Observed),
        },
        BoundingBox::new(0.1, 0.2, 0.8, 0.05),
        3,
        42,
    )
    .with_hint(SemanticHint::from_format(HintKind::Heading { level: 1 }));

    let json = serde_json::to_string(&prim).unwrap();
    let deserialized: Primitive = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.text_content(), Some("Hello world"));
    assert_eq!(deserialized.page, 3);
    assert_eq!(deserialized.source_order, 42);
    assert_eq!(deserialized.hints.len(), 1);
    assert!(matches!(
        deserialized.hints[0].kind,
        HintKind::Heading { level: 1 }
    ));
}

#[test]
fn serde_roundtrip_primitive_image() {
    let prim = Primitive::observed(
        PrimitiveKind::Image {
            format: ImageFormat::Png,
            data: vec![0x89, 0x50, 0x4E, 0x47],
            alt_text: Some("Logo".to_string()),
        },
        BoundingBox::new(0.0, 0.0, 0.5, 0.3),
        0,
        0,
    );

    let json = serde_json::to_string(&prim).unwrap();
    let deserialized: Primitive = serde_json::from_str(&json).unwrap();
    match &deserialized.kind {
        PrimitiveKind::Image {
            format,
            data,
            alt_text,
        } => {
            assert_eq!(*format, ImageFormat::Png);
            assert_eq!(data, &[0x89, 0x50, 0x4E, 0x47]);
            assert_eq!(alt_text.as_deref(), Some("Logo"));
        }
        _ => panic!("Expected Image"),
    }
}

#[test]
fn serde_roundtrip_semantic_hint() {
    let hint = SemanticHint::from_heuristic(
        HintKind::TableCell {
            row: 2,
            col: 3,
            rowspan: 1,
            colspan: 2,
        },
        0.85,
        "test-detector",
    );
    let json = serde_json::to_string(&hint).unwrap();
    let deserialized: SemanticHint = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.confidence, 0.85);
    assert!(matches!(
        deserialized.source,
        HintSource::HeuristicInferred { .. }
    ));
    assert!(matches!(
        deserialized.kind,
        HintKind::TableCell {
            row: 2,
            col: 3,
            rowspan: 1,
            colspan: 2
        }
    ));
}

#[test]
fn serde_roundtrip_document_metadata() {
    let meta = DocumentMetadata {
        format: DocumentFormat::Html,
        page_count: 1,
        page_count_provenance: PaginationProvenance::synthetic(PaginationBasis::SingleLogicalPage),
        page_dimensions: vec![],
        encrypted: false,
        text_extraction_allowed: true,
        file_size: 12345,
        title: Some("Test".to_string()),
    };
    let json = serde_json::to_string(&meta).unwrap();
    let deserialized: DocumentMetadata = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.format, DocumentFormat::Html);
    assert_eq!(deserialized.title, Some("Test".to_string()));
}
