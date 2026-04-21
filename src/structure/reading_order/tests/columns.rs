use super::*;

#[test]
fn column_aware_two_columns_ltr() {
    let mut prims = vec![
        text_at(0.55, 0.1, 0.35, "R1", 0),
        text_at(0.05, 0.3, 0.35, "L2", 1),
        text_at(0.55, 0.3, 0.35, "R2", 2),
        text_at(0.05, 0.1, 0.35, "L1", 3),
    ];

    ColumnAwareResolver::default().reorder(&mut prims);

    assert_eq!(texts(&prims), vec!["L1", "L2", "R1", "R2"]);
}

#[test]
fn column_aware_two_columns_rtl() {
    let mut prims = vec![
        text_at_dir(0.55, 0.1, 0.35, "R1", 0, TextDirection::RightToLeft),
        text_at_dir(0.05, 0.3, 0.35, "L2", 1, TextDirection::RightToLeft),
        text_at_dir(0.55, 0.3, 0.35, "R2", 2, TextDirection::RightToLeft),
        text_at_dir(0.05, 0.1, 0.35, "L1", 3, TextDirection::RightToLeft),
    ];

    let resolver = ColumnAwareResolver {
        column_gap_threshold: 0.03,
        text_direction: TextDirection::RightToLeft,
    };
    resolver.reorder(&mut prims);

    assert_eq!(texts(&prims), vec!["R1", "R2", "L1", "L2"]);
}

#[test]
fn column_aware_three_columns() {
    let mut prims = vec![
        text_at(0.70, 0.1, 0.25, "C3_top", 0),
        text_at(0.70, 0.3, 0.25, "C3_bot", 1),
        text_at(0.05, 0.1, 0.25, "C1_top", 2),
        text_at(0.05, 0.3, 0.25, "C1_bot", 3),
        text_at(0.38, 0.1, 0.25, "C2_top", 4),
        text_at(0.38, 0.3, 0.25, "C2_bot", 5),
    ];

    ColumnAwareResolver::default().reorder(&mut prims);

    assert_eq!(
        texts(&prims),
        vec!["C1_top", "C1_bot", "C2_top", "C2_bot", "C3_top", "C3_bot"]
    );
}

#[test]
fn column_aware_single_column_fallback() {
    let mut prims = vec![
        text_at(0.05, 0.5, 0.9, "bottom", 0),
        text_at(0.05, 0.1, 0.9, "top", 1),
    ];

    ColumnAwareResolver::default().reorder(&mut prims);

    assert_eq!(texts(&prims), vec!["top", "bottom"]);
}

#[test]
fn column_aware_non_text_assigned_to_columns() {
    let mut prims: Vec<Primitive> = vec![
        text_at(0.55, 0.1, 0.35, "R1", 0),
        text_at(0.05, 0.1, 0.35, "L1", 1),
        line_at(0.05, 0.2, 0.35, 2),
        text_at(0.05, 0.3, 0.35, "L2", 3),
    ];

    ColumnAwareResolver::default().reorder(&mut prims);

    let content: Vec<String> = prims
        .iter()
        .map(|p| {
            p.text_content()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "LINE".to_string())
        })
        .collect();
    assert_eq!(content, vec!["L1", "LINE", "L2", "R1"]);
}

#[test]
fn column_detection_respects_gap_threshold() {
    let prims = vec![
        text_at(0.05, 0.1, 0.40, "L1", 0),
        text_at(0.50, 0.1, 0.40, "R1", 1),
    ];

    let wide_gap = ColumnAwareResolver {
        column_gap_threshold: 0.03,
        ..Default::default()
    };
    assert!(wide_gap.detect_columns(&prims).len() >= 2);

    let narrow_gap = ColumnAwareResolver {
        column_gap_threshold: 0.10,
        ..Default::default()
    };
    assert_eq!(narrow_gap.detect_columns(&prims).len(), 1);
}

#[test]
fn narrow_columns_filtered_as_noise() {
    let prims = vec![
        text_at(0.01, 0.1, 0.03, "x", 0),
        text_at(0.01, 0.3, 0.03, "y", 1),
        text_at(0.15, 0.1, 0.35, "A", 2),
        text_at(0.15, 0.3, 0.35, "B", 3),
    ];

    let resolver = ColumnAwareResolver {
        column_gap_threshold: 0.03,
        ..Default::default()
    };
    let cols = resolver.detect_columns(&prims);

    assert_eq!(cols.len(), 1);
}
