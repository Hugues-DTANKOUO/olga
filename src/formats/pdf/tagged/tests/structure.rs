use super::*;

#[test]
fn tagged_context_registers_section_nodes_and_boundaries() {
    let mut section = StructElem::new(StructType::Sect);
    section
        .attributes
        .insert("Title".to_string(), Object::String(b"Overview".to_vec()));

    let mut heading = StructElem::new(StructType::H1);
    heading.add_child(StructChild::MarkedContentRef { mcid: 61, page: 0 });

    let mut paragraph = StructElem::new(StructType::P);
    paragraph.add_child(StructChild::MarkedContentRef { mcid: 62, page: 0 });

    section.add_child(StructChild::StructElem(Box::new(heading)));
    section.add_child(StructChild::StructElem(Box::new(paragraph)));

    let mut tree = StructTreeRoot::new();
    tree.add_root_element(section);

    let (context, summary) = build_tagged_pdf_context(&tree, true);
    assert_eq!(summary.object_ref_count, 0);
    assert_eq!(context.semantic_node_count(), 3);

    let page = context.page_context(0).unwrap();
    assert_eq!(page.semantic_node_ids.len(), 3);

    let tagged = apply_tagged_structure_to_text_spans(
        vec![
            dummy_span(Some(61), "Heading"),
            dummy_span(Some(62), "Body"),
        ],
        page,
    );

    assert!(tagged.spans[0].hints.iter().any(|hint| matches!(
        &hint.kind,
        HintKind::SectionStart { title } if title.as_deref() == Some("Overview")
    )));
    assert!(
        tagged.spans[1]
            .hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::SectionEnd))
    );
}

#[test]
fn tagged_context_captures_object_refs_in_semantic_graph() {
    let mut figure = StructElem::new(StructType::Figure);
    figure.add_child(StructChild::ObjectRef(42, 0));

    let mut tree = StructTreeRoot::new();
    tree.add_root_element(figure);

    let (context, summary) = build_tagged_pdf_context(&tree, true);
    assert_eq!(summary.object_ref_count, 1);
    assert_eq!(context.semantic_node_count(), 1);
    assert_eq!(context.semantic_nodes[0].object_refs, vec![(42, 0)]);
}

#[test]
fn tagged_context_extracts_expanded_text_hint() {
    let mut paragraph = StructElem::new(StructType::P);
    paragraph.expansion = Some("Portable Document Format".to_string());
    paragraph.add_child(StructChild::MarkedContentRef { mcid: 51, page: 0 });

    let mut tree = StructTreeRoot::new();
    tree.add_root_element(paragraph);

    let (context, summary) = build_tagged_pdf_context(&tree, true);
    assert_eq!(summary.expansion_without_mcid, 0);

    let page = context.page_context(0).unwrap();
    let metadata = page.metadata_for_mcid(Some(51));
    assert!(metadata.hints.iter().any(|hint| matches!(
        &hint.kind,
        HintKind::ExpandedText { text } if text == "Portable Document Format"
    )));
}

#[test]
fn tagged_context_reports_expanded_text_without_descendant_mcid() {
    let mut paragraph = StructElem::new(StructType::P);
    paragraph.expansion = Some("Portable Document Format".to_string());

    let mut tree = StructTreeRoot::new();
    tree.add_root_element(paragraph);

    let (_, summary) = build_tagged_pdf_context(&tree, true);
    assert_eq!(summary.expansion_without_mcid, 1);
}
