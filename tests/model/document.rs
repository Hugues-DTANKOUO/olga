use olga::model::*;

#[test]
fn document_node_text_collection() {
    let mut table = DocumentNode::new(
        NodeKind::Table { rows: 2, cols: 2 },
        BoundingBox::new(0.0, 0.0, 1.0, 0.5),
        0.9,
        StructureSource::HintAssembled,
        0..1,
    );
    let mut row1 = DocumentNode::new(
        NodeKind::TableRow,
        BoundingBox::new(0.0, 0.0, 1.0, 0.1),
        0.9,
        StructureSource::HintAssembled,
        0..1,
    );
    row1.add_child(DocumentNode::new(
        NodeKind::TableCell {
            row: 0,
            col: 0,
            rowspan: 1,
            colspan: 1,
            text: "A1".to_string(),
        },
        BoundingBox::new(0.0, 0.0, 0.5, 0.1),
        0.9,
        StructureSource::HintAssembled,
        0..1,
    ));
    row1.add_child(DocumentNode::new(
        NodeKind::TableCell {
            row: 0,
            col: 1,
            rowspan: 1,
            colspan: 1,
            text: "B1".to_string(),
        },
        BoundingBox::new(0.5, 0.0, 0.5, 0.1),
        0.9,
        StructureSource::HintAssembled,
        0..1,
    ));
    table.add_child(row1);
    assert_eq!(table.all_text(), "A1 B1");
    assert_eq!(table.node_count(), 4);
    assert_eq!(table.depth(), 3);
}

#[test]
fn document_node_cross_page() {
    let node = DocumentNode::new(
        NodeKind::Table { rows: 10, cols: 3 },
        BoundingBox::new(0.05, 0.1, 0.9, 0.8),
        0.85,
        StructureSource::HintAssembled,
        3..6,
    );
    assert!(node.is_cross_page());
}

#[test]
fn document_node_single_page() {
    let node = DocumentNode::new(
        NodeKind::Paragraph {
            text: "Hello".to_string(),
        },
        BoundingBox::new(0.1, 0.1, 0.8, 0.05),
        1.0,
        StructureSource::HintAssembled,
        0..1,
    );
    assert!(!node.is_cross_page());
}

#[test]
fn document_metadata_processable() {
    let meta = DocumentMetadata {
        format: DocumentFormat::Pdf,
        page_count: 10,
        page_count_provenance: PaginationProvenance::observed(PaginationBasis::PhysicalPages),
        page_dimensions: vec![],
        encrypted: false,
        text_extraction_allowed: true,
        file_size: 1_000_000,
        title: Some("Test".to_string()),
    };
    assert!(meta.is_processable());
}

#[test]
fn document_metadata_encrypted_no_extraction() {
    let meta = DocumentMetadata {
        format: DocumentFormat::Pdf,
        page_count: 10,
        page_count_provenance: PaginationProvenance::observed(PaginationBasis::PhysicalPages),
        page_dimensions: vec![],
        encrypted: true,
        text_extraction_allowed: false,
        file_size: 1_000_000,
        title: None,
    };
    assert!(!meta.is_processable());
}

#[test]
fn document_metadata_encrypted_with_extraction() {
    let meta = DocumentMetadata {
        format: DocumentFormat::Pdf,
        page_count: 10,
        page_count_provenance: PaginationProvenance::observed(PaginationBasis::PhysicalPages),
        page_dimensions: vec![],
        encrypted: true,
        text_extraction_allowed: true,
        file_size: 1_000_000,
        title: None,
    };
    assert!(meta.is_processable());
}
