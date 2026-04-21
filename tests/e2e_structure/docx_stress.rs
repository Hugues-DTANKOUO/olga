use olga::formats::docx::DocxDecoder;
use olga::model::NodeKind;
use olga::traits::FormatDecoder;

use crate::{all_text_of, build_engine, collect_nodes_by_kind, count_nodes_by_kind};

#[test]
fn e2e_docx_stress_decodes_and_structures() {
    let data = include_bytes!("../corpus/docx/mixed_content_stress.docx").to_vec();
    let decoder = DocxDecoder::new();
    let decode_result = decoder.decode(data).expect("DOCX stress decode failed");

    assert!(
        decode_result.primitives.len() >= 30,
        "stress DOCX should have many primitives, got {}",
        decode_result.primitives.len()
    );

    let engine = build_engine();
    let result = engine.structure(decode_result);

    assert!(matches!(result.root.kind, NodeKind::Document));
    assert!(
        result.root.node_count() >= 20,
        "stress DOCX should produce a complex tree, got {} nodes",
        result.root.node_count()
    );
}

#[test]
fn e2e_docx_stress_has_all_structural_types() {
    let data = include_bytes!("../corpus/docx/mixed_content_stress.docx").to_vec();
    let decoder = DocxDecoder::new();
    let decode_result = decoder.decode(data).expect("DOCX stress decode failed");
    let result = build_engine().structure(decode_result);

    let headings = count_nodes_by_kind(&result.root, &|k| matches!(k, NodeKind::Heading { .. }));
    let paragraphs =
        count_nodes_by_kind(&result.root, &|k| matches!(k, NodeKind::Paragraph { .. }));
    let tables = count_nodes_by_kind(&result.root, &|k| matches!(k, NodeKind::Table { .. }));
    let cells = count_nodes_by_kind(&result.root, &|k| matches!(k, NodeKind::TableCell { .. }));

    assert!(
        headings >= 5,
        "stress DOCX has Title + 3xH1 + 3xH2 = 7+ headings, got {}",
        headings
    );
    assert!(
        paragraphs >= 5,
        "stress DOCX has many body paragraphs, got {}",
        paragraphs
    );
    assert!(tables >= 3, "stress DOCX has 3 tables, got {}", tables);
    assert!(
        cells >= 15,
        "stress DOCX tables have many cells, got {}",
        cells
    );
}

#[test]
fn e2e_docx_stress_table_after_heading_not_paragraphs() {
    let data = include_bytes!("../corpus/docx/mixed_content_stress.docx").to_vec();
    let decoder = DocxDecoder::new();
    let decode_result = decoder.decode(data).expect("DOCX stress decode failed");
    let result = build_engine().structure(decode_result);

    let tables = collect_nodes_by_kind(&result.root, &|k| matches!(k, NodeKind::Table { .. }));
    assert!(
        !tables.is_empty(),
        "tables must exist — classify() must not misroute to Paragraph"
    );

    let cell_texts: Vec<String> =
        collect_nodes_by_kind(&result.root, &|k| matches!(k, NodeKind::TableCell { .. }))
            .iter()
            .filter_map(|n| n.text().map(String::from))
            .collect();

    assert!(
        cell_texts.iter().any(|t| t.contains("Engineering")),
        "'Engineering' should be in a table cell, not a paragraph"
    );
    assert!(
        cell_texts.iter().any(|t| t.contains("$2.8M")),
        "'$2.8M' should be in a table cell"
    );
}

#[test]
fn e2e_docx_stress_text_integrity() {
    let data = include_bytes!("../corpus/docx/mixed_content_stress.docx").to_vec();
    let decoder = DocxDecoder::new();
    let decode_result = decoder.decode(data).expect("DOCX stress decode failed");
    let result = build_engine().structure(decode_result);

    let full = all_text_of(&result.root);

    assert!(
        full.contains("Executive Summary"),
        "'Executive Summary' missing"
    );
    assert!(
        full.contains("Financial Overview"),
        "'Financial Overview' missing"
    );
    assert!(
        full.contains("Risk Assessment"),
        "'Risk Assessment' missing"
    );
    assert!(full.contains("Next Steps"), "'Next Steps' missing");
    assert!(
        full.contains("exceeded targets"),
        "'exceeded targets' missing from executive summary"
    );
    assert!(
        full.contains("forward-looking statements"),
        "bold disclaimer text missing"
    );
    assert!(full.contains("Sales"), "'Sales' missing from table");
    assert!(
        full.contains("Operations"),
        "'Operations' missing from table"
    );
    assert!(
        full.contains("supply chain"),
        "long paragraph about supply chain missing"
    );
    assert!(
        full.contains("vendor contracts"),
        "'vendor contracts' missing from final paragraph"
    );
}

#[test]
fn e2e_docx_stress_table_with_empty_cells_survives() {
    let data = include_bytes!("../corpus/docx/mixed_content_stress.docx").to_vec();
    let decoder = DocxDecoder::new();
    let decode_result = decoder.decode(data).expect("DOCX stress decode failed");
    let result = build_engine().structure(decode_result);

    let full = all_text_of(&result.root);
    assert!(
        full.contains("Supply chain disruption"),
        "'Supply chain disruption' missing from risk matrix"
    );
    assert!(
        full.contains("Talent attrition"),
        "'Talent attrition' missing — empty cells may have broken the row"
    );
    assert!(
        full.contains("Regulatory changes"),
        "'Regulatory changes' missing from risk matrix"
    );
}

#[test]
fn e2e_docx_stress_no_heuristic_detection() {
    let data = include_bytes!("../corpus/docx/mixed_content_stress.docx").to_vec();
    let decoder = DocxDecoder::new();
    let decode_result = decoder.decode(data).expect("DOCX stress decode failed");
    let result = build_engine().structure(decode_result);

    assert!(
        result.heuristic_pages.is_empty(),
        "stress DOCX should not trigger heuristic detection, got pages: {:?}",
        result.heuristic_pages
    );
}
