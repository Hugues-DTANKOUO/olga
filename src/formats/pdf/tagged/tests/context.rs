use super::*;

#[test]
fn tagged_context_extracts_heading_and_paragraph_hints() {
    let mut root = StructElem::new(StructType::Document);

    let mut heading = StructElem::new(StructType::H1);
    heading.add_child(StructChild::MarkedContentRef { mcid: 10, page: 0 });

    let mut paragraph = StructElem::new(StructType::P);
    paragraph.add_child(StructChild::MarkedContentRef { mcid: 11, page: 0 });

    root.add_child(StructChild::StructElem(Box::new(heading)));
    root.add_child(StructChild::StructElem(Box::new(paragraph)));

    let mut tree = StructTreeRoot::new();
    tree.add_root_element(root);

    let (context, _) = build_tagged_pdf_context(&tree, true);
    let page = context.page_context(0).unwrap();

    let h1 = page.metadata_for_mcid(Some(10));
    assert!(
        h1.hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::Heading { level: 1 }))
    );

    let para = page.metadata_for_mcid(Some(11));
    assert!(
        para.hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::Paragraph))
    );
}

#[test]
fn tagged_context_extracts_table_coordinates() {
    let mut table = StructElem::new(StructType::Table);

    let mut row1 = StructElem::new(StructType::TR);
    let mut th1 = StructElem::new(StructType::TH);
    th1.add_child(StructChild::MarkedContentRef { mcid: 1, page: 0 });
    let mut th2 = StructElem::new(StructType::TH);
    th2.add_child(StructChild::MarkedContentRef { mcid: 2, page: 0 });
    row1.add_child(StructChild::StructElem(Box::new(th1)));
    row1.add_child(StructChild::StructElem(Box::new(th2)));

    let mut row2 = StructElem::new(StructType::TR);
    let mut td1 = StructElem::new(StructType::TD);
    td1.add_child(StructChild::MarkedContentRef { mcid: 3, page: 0 });
    let mut td2 = StructElem::new(StructType::TD);
    td2.add_child(StructChild::MarkedContentRef { mcid: 4, page: 0 });
    row2.add_child(StructChild::StructElem(Box::new(td1)));
    row2.add_child(StructChild::StructElem(Box::new(td2)));

    table.add_child(StructChild::StructElem(Box::new(row1)));
    table.add_child(StructChild::StructElem(Box::new(row2)));

    let mut tree = StructTreeRoot::new();
    tree.add_root_element(table);

    let (context, _) = build_tagged_pdf_context(&tree, true);
    let page = context.page_context(0).unwrap();

    assert!(
        page.metadata_for_mcid(Some(1))
            .hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::TableHeader { col: 0 }))
    );
    assert!(
        page.metadata_for_mcid(Some(2))
            .hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::TableHeader { col: 1 }))
    );
    assert!(
        page.metadata_for_mcid(Some(3))
            .hints
            .iter()
            .any(|hint| matches!(
                hint.kind,
                HintKind::TableCell {
                    row: 1,
                    col: 0,
                    rowspan: 1,
                    colspan: 1,
                }
            ))
    );
    assert!(
        page.metadata_for_mcid(Some(4))
            .hints
            .iter()
            .any(|hint| matches!(
                hint.kind,
                HintKind::TableCell {
                    row: 1,
                    col: 1,
                    rowspan: 1,
                    colspan: 1,
                }
            ))
    );
}

#[test]
fn tagged_context_extracts_language_and_sidebar_hint() {
    let mut note = StructElem::new(StructType::Note);
    note.attributes
        .insert("Lang".to_string(), Object::String(b"fr-CA".to_vec()));
    note.add_child(StructChild::MarkedContentRef { mcid: 33, page: 0 });

    let mut tree = StructTreeRoot::new();
    tree.add_root_element(note);

    let (context, _) = build_tagged_pdf_context(&tree, true);
    let page = context.page_context(0).unwrap();
    let metadata = page.metadata_for_mcid(Some(33));

    assert_eq!(metadata.lang.as_deref(), Some("fr-CA"));
    assert!(
        metadata
            .hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::Sidebar))
    );
}

#[test]
fn tagged_context_extracts_list_item_hint_with_observed_numbering() {
    let mut list = StructElem::new(StructType::L);
    list.attributes.insert(
        "ListNumbering".to_string(),
        Object::Name("Decimal".to_string()),
    );
    let mut item = StructElem::new(StructType::LI);
    let mut paragraph = StructElem::new(StructType::P);
    paragraph.add_child(StructChild::MarkedContentRef { mcid: 34, page: 0 });
    item.add_child(StructChild::StructElem(Box::new(paragraph)));
    list.add_child(StructChild::StructElem(Box::new(item)));

    let mut tree = StructTreeRoot::new();
    tree.add_root_element(list);

    let (context, _) = build_tagged_pdf_context(&tree, true);
    let page = context.page_context(0).unwrap();
    let metadata = page.metadata_for_mcid(Some(34));

    assert!(metadata.hints.iter().any(|hint| {
        matches!(
            hint.kind,
            HintKind::ListItem {
                depth: 0,
                ordered: true,
                ..
            }
        ) && matches!(hint.source, crate::model::HintSource::FormatDerived)
    }));
    assert!(
        metadata
            .hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::Paragraph))
    );
}

#[test]
fn tagged_context_extracts_document_title_from_source_role() {
    let mut heading = StructElem::new(StructType::H1);
    heading.source_role = Some("Title".to_string());
    heading.add_child(StructChild::MarkedContentRef { mcid: 35, page: 0 });

    let mut tree = StructTreeRoot::new();
    tree.add_root_element(heading);

    let (context, _) = build_tagged_pdf_context(&tree, true);
    let page = context.page_context(0).unwrap();
    let metadata = page.metadata_for_mcid(Some(35));

    assert!(
        metadata
            .hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::DocumentTitle))
    );
    assert!(
        metadata
            .hints
            .iter()
            .any(|hint| matches!(hint.kind, HintKind::Heading { level: 1 }))
    );
}

#[test]
fn tagged_context_extracts_link_hint_from_link_attributes() {
    let mut link = StructElem::new(StructType::Link);
    link.attributes.insert(
        "URI".to_string(),
        Object::String(b"https://example.com".to_vec()),
    );
    link.add_child(StructChild::MarkedContentRef { mcid: 36, page: 0 });

    let mut tree = StructTreeRoot::new();
    tree.add_root_element(link);

    let (context, _) = build_tagged_pdf_context(&tree, true);
    let page = context.page_context(0).unwrap();
    let metadata = page.metadata_for_mcid(Some(36));

    assert!(metadata.hints.iter().any(|hint| matches!(
        &hint.kind,
        HintKind::Link { url } if url == "https://example.com"
    )));
}
