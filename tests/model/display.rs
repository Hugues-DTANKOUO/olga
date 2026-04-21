use olga::model::*;

#[test]
fn display_bounding_box() {
    let bb = BoundingBox::new(0.123, 0.456, 0.789, 0.012);
    let s = format!("{}", bb);
    assert!(s.contains("0.123"));
    assert!(s.contains("0.456"));
}

#[test]
fn display_primitive_text() {
    let prim = Primitive::observed(
        PrimitiveKind::Text {
            content: "Short text".to_string(),
            font_size: 12.0,
            font_name: "Arial".to_string(),
            is_bold: false,
            is_italic: false,
            color: None,
            char_positions: None,
            text_direction: TextDirection::LeftToRight,
            text_direction_origin: ValueOrigin::Observed,
            lang: None,
            lang_origin: None,
        },
        BoundingBox::new(0.1, 0.1, 0.8, 0.05),
        3,
        100,
    );
    let s = format!("{}", prim);
    assert!(s.contains("Text(Short text)"));
    assert!(s.contains("p3"));
}

#[test]
fn display_color() {
    assert_eq!(format!("{}", Color::rgb(255, 128, 0)), "#ff8000");
    assert_eq!(format!("{}", Color::rgba(255, 128, 0, 128)), "#ff800080");
}

#[test]
fn display_document_node_tree() {
    let mut doc = DocumentNode::new(
        NodeKind::Document,
        BoundingBox::new(0.0, 0.0, 1.0, 1.0),
        1.0,
        StructureSource::HintAssembled,
        0..3,
    );
    doc.add_child(DocumentNode::new(
        NodeKind::Heading {
            level: 1,
            text: "Title".to_string(),
        },
        BoundingBox::new(0.1, 0.02, 0.8, 0.04),
        1.0,
        StructureSource::HintAssembled,
        0..1,
    ));
    doc.add_child(DocumentNode::new(
        NodeKind::Paragraph {
            text: "Body text".to_string(),
        },
        BoundingBox::new(0.1, 0.1, 0.8, 0.05),
        0.85,
        StructureSource::HintAssembled,
        0..1,
    ));
    let s = format!("{}", doc);
    assert!(s.contains("Document"));
    assert!(s.contains("H1(Title)"));
    assert!(s.contains("Para(Body text)"));
    assert!(s.contains("85%"));
}
