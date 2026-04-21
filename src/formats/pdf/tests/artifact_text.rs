use super::*;

#[test]
fn repeated_tagged_residual_rules_require_cross_page_repetition() {
    let candidates = vec![
        HeuristicArtifactCandidate {
            page: 0,
            mcid: None,
            text: "Confidential".to_string(),
            band: ArtifactBand::Top,
        },
        HeuristicArtifactCandidate {
            page: 1,
            mcid: None,
            text: "Confidential".to_string(),
            band: ArtifactBand::Top,
        },
        HeuristicArtifactCandidate {
            page: 1,
            mcid: None,
            text: "Appendix".to_string(),
            band: ArtifactBand::Bottom,
        },
    ];

    let mut plan = TextArtifactSuppressionPlan::default();
    add_repeated_tagged_residual_rules(&mut plan, &candidates);

    assert_eq!(plan.rules_for_page(0).unwrap().len(), 1);
    assert_eq!(plan.rules_for_page(1).unwrap().len(), 1);
    assert!(
        plan.rules_for_page(1)
            .unwrap()
            .iter()
            .all(|rule| rule.text != "Appendix")
    );
}

#[test]
fn suppress_artifact_text_spans_keeps_body_text_with_same_content() {
    let rules = vec![TextArtifactRule {
        mcid: None,
        text: "Confidential".to_string(),
        band: ArtifactBand::Top,
        source: ArtifactRuleSource::RepeatedTaggedResidual,
    }];

    let spans = vec![
        dummy_internal_span("Confidential", BoundingBox::new(0.1, 0.02, 0.3, 0.03), None),
        dummy_internal_span("Confidential", BoundingBox::new(0.1, 0.45, 0.3, 0.03), None),
    ];

    let (filtered, stats) = suppress_artifact_text_spans(spans, Some(&rules));

    assert_eq!(stats.repeated_tagged_residual, 1);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].content, "Confidential");
    assert_eq!(filtered[0].bbox.y, 0.45);
}

#[test]
fn suppress_artifact_text_spans_matches_explicit_mcid_rules() {
    let rules = vec![TextArtifactRule {
        mcid: Some(7),
        text: "anything".to_string(),
        band: ArtifactBand::Bottom,
        source: ArtifactRuleSource::ExplicitTaggedPagination,
    }];

    let spans = vec![
        dummy_internal_span("Footer", BoundingBox::new(0.1, 0.92, 0.2, 0.03), Some(7)),
        dummy_internal_span("Body", BoundingBox::new(0.1, 0.30, 0.2, 0.03), Some(8)),
    ];

    let (filtered, stats) = suppress_artifact_text_spans(spans, Some(&rules));

    assert_eq!(stats.explicit_tagged_pagination, 1);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].content, "Body");
}

#[test]
fn extract_artifact_type_from_properties_dict_handles_pagination_subtypes() {
    let mut dict = HashMap::new();
    dict.insert("Type".to_string(), Object::Name("Pagination".to_string()));
    dict.insert("Subtype".to_string(), Object::Name("Watermark".to_string()));

    assert_eq!(
        extract_artifact_type_from_properties_dict(&Object::Dictionary(dict)),
        Some(ArtifactType::Pagination(PaginationSubtype::Watermark))
    );
}

#[test]
fn active_marked_content_artifact_survives_nested_semantic_content() {
    let stack = vec![
        MarkedContentContext {
            mcid: None,
            is_artifact: true,
            artifact_type: None,
        },
        MarkedContentContext {
            mcid: Some(17),
            is_artifact: false,
            artifact_type: None,
        },
    ];

    let active_artifact = active_marked_content_artifact(&stack).unwrap();
    assert_eq!(active_marked_content_mcid(&stack), Some(17));
    assert!(active_artifact.is_artifact);
    assert_eq!(active_artifact.artifact_type, None);
}
