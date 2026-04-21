use super::*;

#[test]
fn empty_primitives_reorder_is_noop() {
    let mut prims: Vec<Primitive> = vec![];

    TaggedOrderResolver.reorder(&mut prims);
    SimpleResolver::default().reorder(&mut prims);
    ColumnAwareResolver::default().reorder(&mut prims);

    assert!(prims.is_empty());
}

#[test]
fn single_primitive_unchanged() {
    let mut prims = vec![text_at(0.5, 0.5, 0.2, "alone", 0)];

    ColumnAwareResolver::default().reorder(&mut prims);

    assert_eq!(texts(&prims), vec!["alone"]);
}

#[test]
fn detect_dominant_direction_works() {
    let prims = vec![
        text_at_dir(0.05, 0.1, 0.3, "a", 0, TextDirection::RightToLeft),
        text_at_dir(0.05, 0.2, 0.3, "b", 1, TextDirection::RightToLeft),
        text_at_dir(0.05, 0.3, 0.3, "c", 2, TextDirection::LeftToRight),
    ];

    let dir = detect_dominant_direction(&prims);
    assert_eq!(dir, TextDirection::RightToLeft);
}

#[test]
fn all_non_text_page_falls_back_to_simple_order() {
    use crate::model::Point;

    let mut prims: Vec<Primitive> = vec![
        Primitive::observed(
            PrimitiveKind::Line {
                start: Point::new(0.05, 0.5),
                end: Point::new(0.95, 0.5),
                thickness: 0.001,
                color: None,
            },
            BoundingBox::new(0.05, 0.5, 0.9, 0.001),
            0,
            0,
        ),
        Primitive::observed(
            PrimitiveKind::Line {
                start: Point::new(0.05, 0.1),
                end: Point::new(0.95, 0.1),
                thickness: 0.001,
                color: None,
            },
            BoundingBox::new(0.05, 0.1, 0.9, 0.001),
            0,
            1,
        ),
        Primitive::observed(
            PrimitiveKind::Line {
                start: Point::new(0.05, 0.3),
                end: Point::new(0.95, 0.3),
                thickness: 0.001,
                color: None,
            },
            BoundingBox::new(0.05, 0.3, 0.9, 0.001),
            0,
            2,
        ),
    ];

    ColumnAwareResolver::default().reorder(&mut prims);

    assert!((prims[0].bbox.y - 0.1).abs() < 0.001);
    assert!((prims[1].bbox.y - 0.3).abs() < 0.001);
    assert!((prims[2].bbox.y - 0.5).abs() < 0.001);
}
