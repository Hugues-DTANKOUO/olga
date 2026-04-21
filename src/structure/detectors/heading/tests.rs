use super::helpers::build_level_map;
use super::*;
use crate::model::{BoundingBox, HintKind, Primitive, PrimitiveKind, SemanticHint};
use crate::structure::detectors::{DetectionContext, StructureDetector};
use crate::structure::types::FontHistogram;

fn text_sized(y: f32, font_size: f32, bold: bool, content: &str, order: u64) -> Primitive {
    Primitive::observed(
        PrimitiveKind::Text {
            content: content.to_string(),
            font_size,
            font_name: "Arial".to_string(),
            is_bold: bold,
            is_italic: false,
            color: None,
            char_positions: None,
            text_direction: Default::default(),
            text_direction_origin: crate::model::provenance::ValueOrigin::Observed,
            lang: None,
            lang_origin: None,
        },
        BoundingBox::new(0.05, y, 0.9, 0.03),
        0,
        order,
    )
}

fn ctx_with_body(body_count: u64, body_size: f32) -> DetectionContext {
    let mut hist = FontHistogram::default();
    for _ in 0..body_count {
        hist.record(body_size);
    }
    DetectionContext {
        font_histogram: hist,
        page: 0,
        page_dims: None,
        text_boxes: None,
        open_table_columns: None,
    }
}

#[test]
fn large_bold_text_detected_as_heading() {
    let page = vec![
        text_sized(0.05, 18.0, true, "Chapter One", 0),
        text_sized(
            0.10,
            12.0,
            false,
            "Body text here and more text to make it longer.",
            1,
        ),
    ];

    let mut ctx = ctx_with_body(100, 12.0);
    ctx.font_histogram.record(18.0);

    let detector = HeadingDetector::default();
    let hints = detector.detect(&page, &ctx);

    assert_eq!(hints.len(), 1);
    assert_eq!(hints[0].primitive_index, 0);
    match &hints[0].hint.kind {
        HintKind::Heading { level } => assert_eq!(level, &1),
        other => panic!("expected Heading, got {:?}", other),
    }
    assert!((hints[0].hint.confidence - 0.8).abs() < 0.01);
}

#[test]
fn large_non_bold_lower_confidence() {
    let page = vec![text_sized(0.05, 18.0, false, "Subtitle Text", 0)];
    let mut ctx = ctx_with_body(100, 12.0);
    ctx.font_histogram.record(18.0);

    let detector = HeadingDetector::default();
    let hints = detector.detect(&page, &ctx);

    assert_eq!(hints.len(), 1);
    assert!((hints[0].hint.confidence - 0.5).abs() < 0.01);
}

#[test]
fn body_size_text_not_detected() {
    let page = vec![
        text_sized(0.05, 12.0, false, "Normal body text.", 0),
        text_sized(0.10, 12.0, true, "Bold body text but same size.", 1),
    ];
    let ctx = ctx_with_body(100, 12.0);

    let detector = HeadingDetector::default();
    let hints = detector.detect(&page, &ctx);

    assert!(hints.is_empty());
}

#[test]
fn multiple_heading_levels() {
    let page = vec![
        text_sized(0.05, 24.0, true, "Main Title Here", 0),
        text_sized(0.10, 18.0, true, "Subtitle Here", 1),
        text_sized(0.15, 12.0, false, "Body text here and more.", 2),
    ];

    let mut ctx = ctx_with_body(100, 12.0);
    ctx.font_histogram.record(24.0);
    ctx.font_histogram.record(18.0);

    let detector = HeadingDetector::default();
    let hints = detector.detect(&page, &ctx);

    assert_eq!(hints.len(), 2);

    match &hints[0].hint.kind {
        HintKind::Heading { level } => assert_eq!(level, &1),
        other => panic!("expected H1, got {:?}", other),
    }

    match &hints[1].hint.kind {
        HintKind::Heading { level } => assert_eq!(level, &2),
        other => panic!("expected H2, got {:?}", other),
    }
}

#[test]
fn short_text_filtered_out() {
    let page = vec![text_sized(0.05, 24.0, true, "A", 0)];
    let mut ctx = ctx_with_body(100, 12.0);
    ctx.font_histogram.record(24.0);

    let detector = HeadingDetector::default();
    let hints = detector.detect(&page, &ctx);

    assert!(hints.is_empty());
}

#[test]
fn long_text_filtered_out() {
    let long = "A".repeat(250);
    let page = vec![text_sized(0.05, 24.0, true, &long, 0)];
    let mut ctx = ctx_with_body(100, 12.0);
    ctx.font_histogram.record(24.0);

    let detector = HeadingDetector::default();
    let hints = detector.detect(&page, &ctx);

    assert!(hints.is_empty());
}

#[test]
fn already_hinted_primitives_skipped() {
    let page = vec![
        text_sized(0.05, 24.0, true, "Already Hinted", 0)
            .with_hint(SemanticHint::from_format(HintKind::Heading { level: 1 })),
    ];
    let mut ctx = ctx_with_body(100, 12.0);
    ctx.font_histogram.record(24.0);

    let detector = HeadingDetector::default();
    let hints = detector.detect(&page, &ctx);

    assert!(hints.is_empty());
}

#[test]
fn empty_histogram_returns_no_hints() {
    let page = vec![text_sized(0.05, 24.0, true, "Big Text", 0)];
    let ctx = DetectionContext {
        font_histogram: FontHistogram::default(),
        page: 0,
        page_dims: None,
        text_boxes: None,
        open_table_columns: None,
    };

    let detector = HeadingDetector::default();
    let hints = detector.detect(&page, &ctx);

    assert!(hints.is_empty());
}

#[test]
fn level_map_caps_at_h6() {
    let map = [
        (30.0, 0),
        (28.0, 0),
        (26.0, 0),
        (24.0, 0),
        (22.0, 0),
        (20.0, 0),
        (18.0, 0),
    ];
    let page: Vec<Primitive> = map
        .iter()
        .enumerate()
        .map(|(i, (size, _))| text_sized(i as f32 * 0.04, *size, true, "Heading text", i as u64))
        .collect();

    let level_map = build_level_map(&page, 12.0, 1.15);
    assert!(level_map.len() == 7);
    assert_eq!(level_map[6].1, 6);
}

#[test]
fn validate_clamps_invalid_percentile() {
    let mut d = HeadingDetector {
        percentile: 2.0,
        ..Default::default()
    };
    let issues = d.validate();
    assert!(!issues.is_empty(), "should report clamped percentile");
    assert!((d.percentile - 0.10).abs() < f32::EPSILON);
}

#[test]
fn validate_clamps_invalid_min_size_ratio() {
    let mut d = HeadingDetector {
        min_size_ratio: 0.5,
        ..Default::default()
    };
    let issues = d.validate();
    assert!(!issues.is_empty());
    assert!((d.min_size_ratio - 1.0).abs() < f32::EPSILON);
}

#[test]
fn validate_accepts_valid_config() {
    let mut d = HeadingDetector::default();
    let issues = d.validate();
    assert!(issues.is_empty(), "default config should be valid");
}
