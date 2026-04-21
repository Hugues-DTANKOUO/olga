use super::*;

#[test]
fn simple_resolver_sorts_top_to_bottom_ltr() {
    let mut prims = vec![
        text_at(0.05, 0.5, 0.4, "bottom", 0),
        text_at(0.05, 0.1, 0.4, "top", 1),
        text_at(0.05, 0.3, 0.4, "middle", 2),
    ];

    SimpleResolver::default().reorder(&mut prims);

    assert_eq!(texts(&prims), vec!["top", "middle", "bottom"]);
}

#[test]
fn simple_resolver_breaks_ties_left_to_right() {
    let mut prims = vec![
        text_at(0.5, 0.1, 0.2, "right", 0),
        text_at(0.05, 0.1, 0.2, "left", 1),
    ];

    SimpleResolver::default().reorder(&mut prims);

    assert_eq!(texts(&prims), vec!["left", "right"]);
}

#[test]
fn simple_resolver_rtl_breaks_ties_right_to_left() {
    let mut prims = vec![
        text_at(0.05, 0.1, 0.2, "left", 0),
        text_at(0.5, 0.1, 0.2, "right", 1),
    ];

    let resolver = SimpleResolver {
        text_direction: TextDirection::RightToLeft,
    };
    resolver.reorder(&mut prims);

    assert_eq!(texts(&prims), vec!["right", "left"]);
}
