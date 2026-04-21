use olga::model::*;

#[test]
fn primitive_new_text() {
    let prim = Primitive::observed(
        PrimitiveKind::Text {
            content: "Hello world".to_string(),
            font_size: 12.0,
            font_name: "Arial".to_string(),
            is_bold: false,
            is_italic: false,
            color: Some(Color::black()),
            char_positions: None,
            text_direction: TextDirection::LeftToRight,
            text_direction_origin: ValueOrigin::Observed,
            lang: None,
            lang_origin: None,
        },
        BoundingBox::new(0.1, 0.1, 0.8, 0.05),
        0,
        1,
    );
    assert!(prim.is_text());
    assert!(!prim.is_line());
    assert_eq!(prim.text_content(), Some("Hello world"));
    assert!(!prim.has_format_hints());
}

#[test]
fn primitive_with_hints() {
    let prim = Primitive::observed(
        PrimitiveKind::Text {
            content: "Chapter 1".to_string(),
            font_size: 24.0,
            font_name: "Arial".to_string(),
            is_bold: true,
            is_italic: false,
            color: None,
            char_positions: None,
            text_direction: TextDirection::LeftToRight,
            text_direction_origin: ValueOrigin::Observed,
            lang: None,
            lang_origin: None,
        },
        BoundingBox::new(0.1, 0.05, 0.8, 0.04),
        0,
        0,
    )
    .with_hint(SemanticHint::from_format(HintKind::Heading { level: 1 }))
    .with_hint(SemanticHint::from_format(HintKind::DocumentTitle));

    assert_eq!(prim.hints.len(), 2);
    assert!(prim.has_format_hints());
}

#[test]
fn hint_from_format_confidence_is_one() {
    let h = SemanticHint::from_format(HintKind::Paragraph);
    assert!((h.confidence - 1.0).abs() < f32::EPSILON);
    assert_eq!(h.source, HintSource::FormatDerived);
}

#[test]
fn hint_from_heuristic_clamps() {
    let h = SemanticHint::from_heuristic(HintKind::Heading { level: 2 }, 1.5, "test-detector");
    assert!((h.confidence - 1.0).abs() < f32::EPSILON);
    let h2 = SemanticHint::from_heuristic(HintKind::Heading { level: 2 }, -0.5, "test-detector");
    assert!((h2.confidence - 0.0).abs() < f32::EPSILON);
}
