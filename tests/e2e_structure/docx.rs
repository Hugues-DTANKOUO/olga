use olga::formats::docx::DocxDecoder;
use olga::model::{HintKind, NodeKind, StructureSource};
use olga::traits::FormatDecoder;

use crate::{all_text_of, build_engine, collect_nodes_by_kind};

#[test]
fn e2e_docx_project_status_decodes_and_structures() {
    let data = include_bytes!("../corpus/docx/project_status.docx").to_vec();
    let decoder = DocxDecoder::new();
    let decode_result = decoder.decode(data).expect("DOCX decode should succeed");

    assert!(
        !decode_result.primitives.is_empty(),
        "should extract primitives"
    );

    let engine = build_engine();
    let result = engine.structure(decode_result);

    assert!(matches!(result.root.kind, NodeKind::Document));
    assert!(
        !result.root.children.is_empty(),
        "document should have children"
    );
}

#[test]
fn e2e_docx_project_status_has_headings() {
    let data = include_bytes!("../corpus/docx/project_status.docx").to_vec();
    let decoder = DocxDecoder::new();
    let decode_result = decoder.decode(data).expect("DOCX decode should succeed");
    let engine = build_engine();
    let result = engine.structure(decode_result);

    let headings = collect_nodes_by_kind(&result.root, &|k| matches!(k, NodeKind::Heading { .. }));

    assert!(
        headings.len() >= 4,
        "expected at least 4 headings, got {}",
        headings.len()
    );

    let heading_texts: Vec<&str> = headings.iter().filter_map(|n| n.text()).collect();
    assert!(
        heading_texts
            .iter()
            .any(|t| t.contains("Executive Summary")),
        "should contain 'Executive Summary' heading, got: {:?}",
        heading_texts
    );
    assert!(
        heading_texts.iter().any(|t| t.contains("Next Steps")),
        "should contain 'Next Steps' heading, got: {:?}",
        heading_texts
    );
}

#[test]
fn e2e_docx_project_status_has_paragraphs() {
    let data = include_bytes!("../corpus/docx/project_status.docx").to_vec();
    let decoder = DocxDecoder::new();
    let decode_result = decoder.decode(data).expect("DOCX decode should succeed");
    let engine = build_engine();
    let result = engine.structure(decode_result);

    let paragraphs =
        collect_nodes_by_kind(&result.root, &|k| matches!(k, NodeKind::Paragraph { .. }));

    assert!(
        paragraphs.len() >= 3,
        "expected at least 3 paragraphs, got {}",
        paragraphs.len()
    );

    let full_text = all_text_of(&result.root);
    assert!(
        full_text.contains("stakeholder"),
        "full text should mention 'stakeholder'"
    );
}

#[test]
fn e2e_docx_project_status_has_table_content() {
    let data = include_bytes!("../corpus/docx/project_status.docx").to_vec();
    let decoder = DocxDecoder::new();
    let decode_result = decoder.decode(data).expect("DOCX decode should succeed");

    let has_table_hints = decode_result.primitives.iter().any(|p| {
        p.hints.iter().any(|h| {
            matches!(
                h.kind,
                HintKind::TableCell { .. } | HintKind::TableHeader { .. }
            )
        })
    });
    assert!(
        has_table_hints,
        "DOCX decoder should emit TableCell/TableHeader hints for the table"
    );

    let engine = build_engine();
    let result = engine.structure(decode_result);

    let tables = collect_nodes_by_kind(&result.root, &|k| matches!(k, NodeKind::Table { .. }));
    assert!(
        !tables.is_empty(),
        "DOCX table should produce Table nodes (classify priority fix)"
    );

    let cells = collect_nodes_by_kind(&result.root, &|k| matches!(k, NodeKind::TableCell { .. }));
    assert!(
        cells.len() >= 4,
        "expected at least 4 table cells, got {}",
        cells.len()
    );

    let full_text = all_text_of(&result.root);
    assert!(
        full_text.contains("Development"),
        "table cell 'Development' should appear in output"
    );
    assert!(
        full_text.contains("Infrastructure"),
        "table cell 'Infrastructure' should appear in output"
    );
}

#[test]
fn e2e_docx_project_status_has_list_content() {
    let data = include_bytes!("../corpus/docx/project_status.docx").to_vec();
    let decoder = DocxDecoder::new();
    let decode_result = decoder.decode(data).expect("DOCX decode should succeed");
    let engine = build_engine();
    let result = engine.structure(decode_result);

    let full_text = all_text_of(&result.root);
    assert!(
        full_text.contains("Security audit"),
        "list bullet text 'Security audit' should appear in output"
    );
    assert!(
        full_text.contains("Requirements finalization"),
        "list bullet text 'Requirements finalization' should appear in output"
    );
    assert!(
        full_text.contains("Database schema migration"),
        "list bullet text 'Database schema migration' should appear in output"
    );

    let list_items =
        collect_nodes_by_kind(&result.root, &|k| matches!(k, NodeKind::ListItem { .. }));
    assert_eq!(
        list_items.len(),
        0,
        "python-docx List Bullet without numPr should produce 0 ListItem nodes"
    );
}

#[test]
fn e2e_docx_project_status_provenance_is_format_derived() {
    let data = include_bytes!("../corpus/docx/project_status.docx").to_vec();
    let decoder = DocxDecoder::new();
    let decode_result = decoder.decode(data).expect("DOCX decode should succeed");
    let engine = build_engine();
    let result = engine.structure(decode_result);

    assert!(
        result.heuristic_pages.is_empty(),
        "DOCX should not trigger heuristic detection, but got pages: {:?}",
        result.heuristic_pages
    );

    let headings = collect_nodes_by_kind(&result.root, &|k| matches!(k, NodeKind::Heading { .. }));
    for heading in &headings {
        assert!(
            matches!(heading.structure_source, StructureSource::HintAssembled),
            "DOCX headings should be HintAssembled, got {:?}",
            heading.structure_source
        );
    }
}
