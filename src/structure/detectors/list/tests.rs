use super::prefix::detect_list_prefix;
use super::*;
use crate::model::{BoundingBox, HintKind, Primitive, PrimitiveKind, SemanticHint};
use crate::structure::detectors::DetectionContext;
use crate::structure::types::FontHistogram;

fn text_at(x: f32, y: f32, content: &str, order: u64) -> Primitive {
    Primitive::observed(
        PrimitiveKind::Text {
            content: content.to_string(),
            font_size: 12.0,
            font_name: "Arial".to_string(),
            is_bold: false,
            is_italic: false,
            color: None,
            char_positions: None,
            text_direction: Default::default(),
            text_direction_origin: crate::model::provenance::ValueOrigin::Observed,
            lang: None,
            lang_origin: None,
        },
        BoundingBox::new(x, y, 0.8, 0.02),
        0,
        order,
    )
}

fn ctx() -> DetectionContext {
    DetectionContext {
        font_histogram: FontHistogram::default(),
        page: 0,
        page_dims: None,
        text_boxes: None,
        open_table_columns: None,
    }
}

#[test]
fn bullet_items_detected() {
    let page = vec![
        text_at(0.05, 0.10, "• First item", 0),
        text_at(0.05, 0.13, "• Second item", 1),
        text_at(0.05, 0.16, "• Third item", 2),
    ];
    let detector = ListDetector::default();
    let hints = detector.detect(&page, &ctx());

    assert_eq!(hints.len(), 3);
    for hint in &hints {
        match &hint.hint.kind {
            HintKind::ListItem { depth, ordered, .. } => {
                assert_eq!(*depth, 0);
                assert!(!ordered);
            }
            other => panic!("expected ListItem, got {:?}", other),
        }
    }
}

#[test]
fn ordered_items_detected() {
    let page = vec![
        text_at(0.05, 0.10, "1. First step", 0),
        text_at(0.05, 0.13, "2. Second step", 1),
        text_at(0.05, 0.16, "3. Third step", 2),
    ];
    let detector = ListDetector::default();
    let hints = detector.detect(&page, &ctx());

    assert_eq!(hints.len(), 3);
    for hint in &hints {
        match &hint.hint.kind {
            HintKind::ListItem { ordered, .. } => assert!(ordered),
            other => panic!("expected ordered ListItem, got {:?}", other),
        }
    }
}

#[test]
fn dash_items_detected() {
    let page = vec![
        text_at(0.05, 0.10, "- Item one", 0),
        text_at(0.05, 0.13, "- Item two", 1),
    ];
    let detector = ListDetector::default();
    let hints = detector.detect(&page, &ctx());

    assert_eq!(hints.len(), 2);
    for hint in &hints {
        assert!(matches!(
            hint.hint.kind,
            HintKind::ListItem { ordered: false, .. }
        ));
    }
}

#[test]
fn parenthesized_numbers_detected() {
    let page = vec![
        text_at(0.05, 0.10, "(1) First", 0),
        text_at(0.05, 0.13, "(2) Second", 1),
    ];
    let detector = ListDetector::default();
    let hints = detector.detect(&page, &ctx());

    assert_eq!(hints.len(), 2);
    for hint in &hints {
        assert!(matches!(
            hint.hint.kind,
            HintKind::ListItem { ordered: true, .. }
        ));
    }
}

#[test]
fn letter_items_detected() {
    let page = vec![
        text_at(0.05, 0.10, "a) First", 0),
        text_at(0.05, 0.13, "b) Second", 1),
    ];
    let detector = ListDetector::default();
    let hints = detector.detect(&page, &ctx());

    assert_eq!(hints.len(), 2);
}

#[test]
fn roman_numeral_items_detected() {
    let page = vec![
        text_at(0.05, 0.10, "i. First point", 0),
        text_at(0.05, 0.13, "ii. Second point", 1),
        text_at(0.05, 0.16, "iii. Third point", 2),
    ];
    let detector = ListDetector::default();
    let hints = detector.detect(&page, &ctx());

    assert_eq!(hints.len(), 3);
}

#[test]
fn non_list_text_ignored() {
    let page = vec![
        text_at(0.05, 0.10, "This is a normal paragraph.", 0),
        text_at(0.05, 0.13, "Another sentence follows.", 1),
    ];
    let detector = ListDetector::default();
    let hints = detector.detect(&page, &ctx());

    assert!(hints.is_empty());
}

#[test]
fn indented_items_get_depth() {
    // Dominant margin at 0.05, indented items at 0.10
    let page = vec![
        text_at(0.05, 0.10, "Normal text.", 0),
        text_at(0.05, 0.13, "• Top-level item", 1),
        text_at(0.10, 0.16, "• Nested item", 2), // indented by 0.05
    ];
    let detector = ListDetector::default();
    let hints = detector.detect(&page, &ctx());

    assert_eq!(hints.len(), 2);
    // First bullet at dominant margin → depth 0.
    match &hints[0].hint.kind {
        HintKind::ListItem { depth, .. } => assert_eq!(*depth, 0),
        other => panic!("expected ListItem, got {:?}", other),
    }
    // Indented bullet → depth > 0.
    match &hints[1].hint.kind {
        HintKind::ListItem { depth, .. } => assert!(*depth > 0),
        other => panic!("expected ListItem, got {:?}", other),
    }
}

#[test]
fn empty_page_no_hints() {
    let page: Vec<Primitive> = vec![];
    let detector = ListDetector::default();
    let hints = detector.detect(&page, &ctx());
    assert!(hints.is_empty());
}

#[test]
fn already_hinted_skipped() {
    let page =
        vec![
            text_at(0.05, 0.10, "• Already hinted", 0).with_hint(SemanticHint::from_format(
                HintKind::ListItem {
                    depth: 0,
                    ordered: false,
                    list_group: None,
                },
            )),
        ];
    let detector = ListDetector::default();
    let hints = detector.detect(&page, &ctx());
    assert!(hints.is_empty());
}

#[test]
fn validate_clamps_invalid_confidence() {
    let mut d = ListDetector {
        prefix_confidence: 2.0,
        ..Default::default()
    };
    let issues = d.validate();
    assert!(!issues.is_empty());
    assert!((d.prefix_confidence - 0.75).abs() < f32::EPSILON);
}

#[test]
fn prefix_helpers() {
    assert!(detect_list_prefix("• Hello").is_some());
    assert!(detect_list_prefix("- Item").is_some());
    assert!(detect_list_prefix("* Item").is_some());
    assert!(detect_list_prefix("1. Step").is_some());
    assert!(detect_list_prefix("a) Choice").is_some());
    assert!(detect_list_prefix("(iv) Roman").is_some());
    assert!(detect_list_prefix("Normal text").is_none());
    assert!(detect_list_prefix("The quick brown fox").is_none());
}

// -- Group ID tests -------------------------------------------------------

#[test]
fn consecutive_bullets_same_group() {
    let page = vec![
        text_at(0.05, 0.10, "• First", 0),
        text_at(0.05, 0.13, "• Second", 1),
        text_at(0.05, 0.16, "• Third", 2),
    ];
    let detector = ListDetector::default();
    let hints = detector.detect(&page, &ctx());

    assert_eq!(hints.len(), 3);
    // All should have the same group ID.
    let groups: Vec<Option<u32>> = hints
        .iter()
        .filter_map(|h| match &h.hint.kind {
            HintKind::ListItem { list_group, .. } => Some(*list_group),
            _ => None,
        })
        .collect();
    assert_eq!(groups, vec![Some(0), Some(0), Some(0)]);
}

#[test]
fn paragraph_between_lists_splits_groups() {
    // Two bullets, then a paragraph, then two more bullets.
    let page = vec![
        text_at(0.05, 0.10, "• Alpha", 0),
        text_at(0.05, 0.13, "• Beta", 1),
        text_at(0.05, 0.16, "A normal paragraph here.", 2), // breaks the group
        text_at(0.05, 0.19, "• Gamma", 3),
        text_at(0.05, 0.22, "• Delta", 4),
    ];
    let detector = ListDetector::default();
    let hints = detector.detect(&page, &ctx());

    assert_eq!(hints.len(), 4);
    let groups: Vec<Option<u32>> = hints
        .iter()
        .filter_map(|h| match &h.hint.kind {
            HintKind::ListItem { list_group, .. } => Some(*list_group),
            _ => None,
        })
        .collect();
    // First two in group 0, last two in group 1.
    assert_eq!(groups[0], Some(0));
    assert_eq!(groups[1], Some(0));
    assert_eq!(groups[2], Some(1));
    assert_eq!(groups[3], Some(1));
}

#[test]
fn mixed_ordered_unordered_splits_groups() {
    // Unordered bullets then ordered numbers should be different groups.
    let page = vec![
        text_at(0.05, 0.10, "• Bullet A", 0),
        text_at(0.05, 0.13, "• Bullet B", 1),
        text_at(0.05, 0.16, "1. Step one", 2),
        text_at(0.05, 0.19, "2. Step two", 3),
    ];
    let detector = ListDetector::default();
    let hints = detector.detect(&page, &ctx());

    assert_eq!(hints.len(), 4);
    let groups: Vec<Option<u32>> = hints
        .iter()
        .filter_map(|h| match &h.hint.kind {
            HintKind::ListItem { list_group, .. } => Some(*list_group),
            _ => None,
        })
        .collect();
    // Bullets in group 0, numbers in group 1.
    assert_eq!(groups[0], Some(0));
    assert_eq!(groups[1], Some(0));
    assert_eq!(groups[2], Some(1));
    assert_eq!(groups[3], Some(1));
}

// -- Indent-only detection tests ------------------------------------------

#[test]
fn indent_only_disabled_by_default() {
    // 4 indented lines with no bullets → no hints when indent-only is disabled.
    let page = vec![
        text_at(0.05, 0.10, "Line one", 0),
        text_at(0.05, 0.13, "Line two", 1),
        text_at(0.05, 0.16, "Line three", 2),
        text_at(0.05, 0.19, "Line four", 3),
    ];
    let detector = ListDetector::default();
    let hints = detector.detect(&page, &ctx());

    // No hints because enable_indent_only is false by default.
    assert!(hints.is_empty());
}

#[test]
fn indent_only_enabled_detects_indented_block() {
    // 4 indented lines (all at same indent > dominant) → 4 ListItem hints
    // when enable_indent_only is true.
    // We need enough items at x=0.05 so that 0.05 is the dominant margin,
    // making the items at x=0.10 genuinely indented (indent = 0.05 > min_indent 0.02).
    let page = vec![
        text_at(0.05, 0.04, "Normal line A", 0),
        text_at(0.05, 0.07, "Normal line B", 1),
        text_at(0.05, 0.10, "Normal line C", 2),
        text_at(0.05, 0.13, "Normal line D", 3),
        text_at(0.05, 0.16, "Normal line E", 4),
        text_at(0.10, 0.20, "Indented line one", 5),
        text_at(0.10, 0.23, "Indented line two", 6),
        text_at(0.10, 0.26, "Indented line three", 7),
        text_at(0.10, 0.29, "Indented line four", 8),
    ];
    let detector = ListDetector {
        enable_indent_only: true,
        ..Default::default()
    };
    let hints = detector.detect(&page, &ctx());

    // Should detect 4 indented items (at indices 5, 6, 7, 8) as a list.
    let indent_items: Vec<_> = hints
        .iter()
        .filter(|h| matches!(h.hint.kind, HintKind::ListItem { .. }))
        .collect();
    assert_eq!(indent_items.len(), 4);

    // All should be unordered and have indent_confidence.
    for hint in &indent_items {
        match &hint.hint.kind {
            HintKind::ListItem { ordered, .. } => {
                assert!(!ordered, "indent-only items should be unordered");
            }
            _ => panic!("expected ListItem"),
        }
        assert!(
            (hint.hint.confidence - 0.55).abs() < 0.01,
            "indent-only items should use indent_confidence (0.55)"
        );
    }
}

#[test]
fn indent_only_requires_three_consecutive() {
    // Only 2 indented lines → no hints (need 3+ consecutive items).
    let page = vec![
        text_at(0.05, 0.10, "Normal text", 0),
        text_at(0.10, 0.13, "Indented line one", 1),
        text_at(0.10, 0.16, "Indented line two", 2),
    ];
    let detector = ListDetector {
        enable_indent_only: true,
        ..Default::default()
    };
    let hints = detector.detect(&page, &ctx());

    // No hints because only 2 indented items (need 3+).
    assert!(hints.is_empty());
}

#[test]
fn indent_only_ignores_dominant_margin() {
    // Lines at dominant margin (not indented) → no hints even with enable_indent_only.
    let page = vec![
        text_at(0.05, 0.10, "Line at margin one", 0),
        text_at(0.05, 0.13, "Line at margin two", 1),
        text_at(0.05, 0.16, "Line at margin three", 2),
        text_at(0.05, 0.19, "Line at margin four", 3),
    ];
    let detector = ListDetector {
        enable_indent_only: true,
        ..Default::default()
    };
    let hints = detector.detect(&page, &ctx());

    // No hints because all items are at the dominant margin (not indented).
    assert!(hints.is_empty());
}

// -- Definition list detection tests ------------------------------------------

#[test]
fn definition_list_disabled_by_default() {
    // "Key: value" lines → no hints when definition lists are disabled.
    let page = vec![
        text_at(0.05, 0.10, "Key: value", 0),
        text_at(0.05, 0.13, "Term: definition", 1),
        text_at(0.05, 0.16, "Name: description", 2),
    ];
    let detector = ListDetector::default();
    let hints = detector.detect(&page, &ctx());

    // No hints because enable_definition_lists is false by default.
    assert!(hints.is_empty());
}

#[test]
fn definition_list_enabled_detects_pattern() {
    // 3 "term: definition" lines → 3 hints when enabled.
    let page = vec![
        text_at(0.05, 0.10, "Apple: A red fruit", 0),
        text_at(0.05, 0.13, "Banana: A yellow fruit", 1),
        text_at(0.05, 0.16, "Cherry: A small red fruit", 2),
    ];
    let detector = ListDetector {
        enable_definition_lists: true,
        ..Default::default()
    };
    let hints = detector.detect(&page, &ctx());

    // Should detect 3 definition items.
    let def_items: Vec<_> = hints
        .iter()
        .filter(|h| matches!(h.hint.kind, HintKind::ListItem { .. }))
        .collect();
    assert_eq!(def_items.len(), 3);

    // All should be unordered with indent_confidence.
    for hint in &def_items {
        match &hint.hint.kind {
            HintKind::ListItem { ordered, .. } => {
                assert!(!ordered, "definition items should be unordered");
            }
            _ => panic!("expected ListItem"),
        }
        assert!(
            (hint.hint.confidence - 0.55).abs() < 0.01,
            "definition items should use indent_confidence (0.55)"
        );
    }
}
