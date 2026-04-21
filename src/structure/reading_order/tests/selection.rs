use super::*;

#[test]
fn select_tagged_when_hints_abundant() {
    let prims = vec![
        hinted_text(0.05, 0.1, "a", 0),
        hinted_text(0.05, 0.2, "b", 1),
        hinted_text(0.05, 0.3, "c", 2),
        hinted_text(0.05, 0.4, "d", 3),
        text_at(0.05, 0.5, 0.4, "e", 4),
    ];

    let resolver = select_resolver(&prims, 0.8, 0.03);
    assert_eq!(resolver.name(), "tagged");
}

#[test]
fn select_simple_when_single_column_no_hints() {
    let prims = vec![
        text_at(0.05, 0.1, 0.9, "line1", 0),
        text_at(0.05, 0.2, 0.9, "line2", 1),
        text_at(0.05, 0.3, 0.9, "line3", 2),
    ];

    let resolver = select_resolver(&prims, 0.8, 0.03);
    assert_eq!(resolver.name(), "simple");
}

#[test]
fn select_column_aware_when_multi_column_no_hints() {
    let prims = vec![
        text_at(0.05, 0.1, 0.35, "L1", 0),
        text_at(0.05, 0.3, 0.35, "L2", 1),
        text_at(0.55, 0.1, 0.35, "R1", 2),
        text_at(0.55, 0.3, 0.35, "R2", 3),
    ];

    let resolver = select_resolver(&prims, 0.8, 0.03);
    assert_eq!(resolver.name(), "column-aware");
}

#[test]
fn select_resolver_empty_returns_tagged() {
    let prims: Vec<Primitive> = vec![];
    let resolver = select_resolver(&prims, 0.8, 0.03);
    assert_eq!(resolver.name(), "tagged");
}

#[test]
fn select_resolver_all_non_text_returns_simple() {
    use crate::model::Point;

    let prims = vec![Primitive::observed(
        PrimitiveKind::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(1.0, 0.0),
            thickness: 0.001,
            color: None,
        },
        BoundingBox::new(0.0, 0.0, 1.0, 0.001),
        0,
        0,
    )];

    let resolver = select_resolver(&prims, 0.8, 0.03);
    assert_eq!(resolver.name(), "simple");
}
