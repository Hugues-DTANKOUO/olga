use super::*;

#[test]
fn tagged_resolver_preserves_order() {
    let mut prims = vec![
        text_at(0.5, 0.1, 0.4, "second_spatially", 0),
        text_at(0.05, 0.5, 0.4, "first_spatially", 1),
    ];

    TaggedOrderResolver.reorder(&mut prims);

    assert_eq!(texts(&prims), vec!["second_spatially", "first_spatially"]);
}
