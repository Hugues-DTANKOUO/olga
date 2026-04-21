use super::*;

#[test]
fn tagged_context_extracts_figure_alt_text_candidates() {
    let mut figure = StructElem::new(StructType::Figure);
    figure.alt_text = Some("Company logo".to_string());
    figure.add_child(StructChild::MarkedContentRef { mcid: 44, page: 0 });

    let mut tree = StructTreeRoot::new();
    tree.add_root_element(figure);

    let (context, summary) = build_tagged_pdf_context(&tree, true);
    assert_eq!(summary.alt_text_without_mcid, 0);

    let page = context.page_context(0).unwrap();
    assert_eq!(
        page.resolve_alt_text_for_mcid(Some(44)),
        ImageAltTextResolution::Unique("Company logo".to_string())
    );
}

#[test]
fn tagged_context_reports_alt_text_without_descendant_mcid() {
    let mut figure = StructElem::new(StructType::Figure);
    figure.alt_text = Some("Decorative icon".to_string());

    let mut tree = StructTreeRoot::new();
    tree.add_root_element(figure);

    let (_, summary) = build_tagged_pdf_context(&tree, true);
    assert_eq!(summary.alt_text_without_mcid, 1);
}

#[test]
fn bind_alt_text_to_image_appearances_requires_matching_counts() {
    let mut page = TaggedPageContext::default();
    page.add_alt_text_candidate(7, "Logo");

    let appearances = vec![TaggedImageAppearance {
        mcid: Some(7),
        is_artifact: false,
        artifact_type: None,
    }];
    let result = bind_alt_text_to_image_appearances(Some(&page), Some(&appearances), 2);

    assert_eq!(
        result,
        ImageAltBinding::CountMismatch {
            appearance_count: 1,
            image_count: 2,
        }
    );
}

#[test]
fn bind_alt_text_to_image_appearances_marks_ambiguous_candidates() {
    let mut page = TaggedPageContext::default();
    page.add_alt_text_candidate(9, "Primary logo");
    page.add_alt_text_candidate(9, "Secondary logo");

    let appearances = vec![TaggedImageAppearance {
        mcid: Some(9),
        is_artifact: false,
        artifact_type: None,
    }];
    let result = bind_alt_text_to_image_appearances(Some(&page), Some(&appearances), 1);

    assert_eq!(
        result,
        ImageAltBinding::Resolved(vec![ImageAltTextResolution::Ambiguous(vec![
            "Primary logo".to_string(),
            "Secondary logo".to_string(),
        ])])
    );
}
