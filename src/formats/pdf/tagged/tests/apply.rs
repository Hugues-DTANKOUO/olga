use super::*;

#[test]
fn apply_tagged_structure_uses_structure_tree_order() {
    let mut page = TaggedPageContext::default();
    let metadata = TaggedContentMetadata::default();
    page.push_reading_item(TaggedReadingItem::Mcid {
        mcid: 8,
        metadata: metadata.clone(),
    });
    page.push_reading_item(TaggedReadingItem::Mcid { mcid: 7, metadata });

    let spans = vec![
        dummy_span(Some(7), "second"),
        dummy_span(None, "untagged"),
        dummy_span(Some(8), "first"),
    ];

    let tagged = apply_tagged_structure_to_text_spans(spans, &page);

    assert_eq!(tagged.spans[0].span.content, "first");
    assert_eq!(tagged.spans[1].span.content, "second");
    assert_eq!(tagged.spans[2].span.content, "untagged");
    assert_eq!(tagged.stats.fallback_span_count, 1);
    assert_eq!(tagged.stats.unmatched_structure_item_count, 0);
}

#[test]
fn apply_tagged_structure_uses_actual_text_replacement() {
    let mut page = TaggedPageContext::default();
    let mut metadata = TaggedContentMetadata::default();
    metadata.add_hint(SemanticHint::from_format(HintKind::Paragraph));
    metadata.lang = Some("en-US".to_string());
    metadata.lang_origin = Some(ValueOrigin::Observed);
    page.push_reading_item(TaggedReadingItem::ActualText {
        text: "Portable Document Format".to_string(),
        mcids: vec![1, 2],
        metadata,
    });

    let spans = vec![dummy_span(Some(1), "PDF"), dummy_span(Some(2), "abbr")];
    let tagged = apply_tagged_structure_to_text_spans(spans, &page);

    assert_eq!(tagged.spans.len(), 1);
    assert_eq!(tagged.stats.actual_text_replacement_count, 1);
    assert_eq!(tagged.spans[0].span.content, "Portable Document Format");
    assert!(tagged.spans[0].span.char_bboxes.is_empty());
    assert_eq!(tagged.spans[0].lang.as_deref(), Some("en-US"));
    assert_eq!(
        tagged.spans[0].geometry_provenance.origin,
        ValueOrigin::Reconstructed
    );
    assert!(
        tagged.spans[0]
            .hints
            .iter()
            .all(|hint| !matches!(hint.kind, HintKind::ExpandedText { .. }))
    );
    assert!(
        tagged.spans[0]
            .hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::Paragraph))
    );
}

#[test]
fn apply_tagged_structure_reports_unmatched_items_and_fallback_spans() {
    let mut page = TaggedPageContext::default();
    page.push_reading_item(TaggedReadingItem::Mcid {
        mcid: 100,
        metadata: TaggedContentMetadata::default(),
    });

    let spans = vec![dummy_span(Some(7), "physical-only")];
    let tagged = apply_tagged_structure_to_text_spans(spans, &page);

    assert_eq!(tagged.stats.unmatched_structure_item_count, 1);
    assert_eq!(tagged.stats.fallback_span_count, 1);
    assert_eq!(tagged.spans.len(), 1);
    assert_eq!(tagged.spans[0].span.content, "physical-only");
}

#[test]
fn apply_tagged_structure_reports_unapplied_structural_end_hints() {
    let mut page = TaggedPageContext::default();
    page.push_reading_item(TaggedReadingItem::StructuralEnd {
        hint: SemanticHint::from_format(HintKind::SectionEnd),
    });

    let tagged = apply_tagged_structure_to_text_spans(vec![dummy_span(Some(7), "body")], &page);

    assert_eq!(tagged.stats.unapplied_structural_hint_count, 1);
    assert_eq!(tagged.spans.len(), 1);
}
