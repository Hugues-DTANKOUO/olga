use olga::formats::xlsx::XlsxDecoder;
use olga::traits::FormatDecoder;

use crate::{all_text_of, build_engine, collect_nodes_by_kind};
use olga::model::NodeKind;

#[test]
fn e2e_xlsx_stress_decodes_and_structures() {
    let data = include_bytes!("../corpus/xlsx/multi_sheet_stress.xlsx").to_vec();
    let decoder = XlsxDecoder::new();
    let decode_result = decoder.decode(data).expect("XLSX stress decode failed");

    assert!(
        decode_result.primitives.len() >= 50,
        "stress XLSX should have many primitives, got {}",
        decode_result.primitives.len()
    );

    let result = build_engine().structure(decode_result);

    assert!(matches!(result.root.kind, NodeKind::Document));
    assert!(
        result.root.node_count() >= 50,
        "stress XLSX should produce a large tree, got {} nodes",
        result.root.node_count()
    );
}

#[test]
fn e2e_xlsx_stress_multiple_tables() {
    let data = include_bytes!("../corpus/xlsx/multi_sheet_stress.xlsx").to_vec();
    let decoder = XlsxDecoder::new();
    let decode_result = decoder.decode(data).unwrap();
    let result = build_engine().structure(decode_result);

    let tables = collect_nodes_by_kind(&result.root, &|k| matches!(k, NodeKind::Table { .. }));
    assert!(
        tables.len() >= 4,
        "5 sheets should produce at least 4 Table nodes, got {}",
        tables.len()
    );
}

#[test]
fn e2e_xlsx_stress_large_sheet_data_intact() {
    let data = include_bytes!("../corpus/xlsx/multi_sheet_stress.xlsx").to_vec();
    let decoder = XlsxDecoder::new();
    let decode_result = decoder.decode(data).unwrap();
    let result = build_engine().structure(decode_result);

    let full = all_text_of(&result.root);
    assert!(
        full.contains("North America"),
        "'North America' missing from Revenue sheet"
    );
    assert!(
        full.contains("Middle East"),
        "'Middle East' missing — last region in Revenue sheet"
    );
    assert!(
        full.contains("Widget A"),
        "'Widget A' missing from Revenue sheet"
    );
    assert!(
        full.contains("Consulting"),
        "'Consulting' missing — last product in Revenue sheet"
    );
}

#[test]
fn e2e_xlsx_stress_sparse_sheet_survives() {
    let data = include_bytes!("../corpus/xlsx/multi_sheet_stress.xlsx").to_vec();
    let decoder = XlsxDecoder::new();
    let decode_result = decoder.decode(data).unwrap();
    let result = build_engine().structure(decode_result);

    let full = all_text_of(&result.root);
    assert!(full.contains("A001"), "'A001' missing from Sparse sheet");
    assert!(
        full.contains("Needs review"),
        "'Needs review' missing from Sparse sheet"
    );
    assert!(full.contains("C003"), "'C003' missing from Sparse sheet");
}

#[test]
fn e2e_xlsx_stress_single_cell_sheet() {
    let data = include_bytes!("../corpus/xlsx/multi_sheet_stress.xlsx").to_vec();
    let decoder = XlsxDecoder::new();
    let decode_result = decoder.decode(data).unwrap();
    let result = build_engine().structure(decode_result);

    let full = all_text_of(&result.root);
    assert!(
        full.contains("OnlyValue"),
        "'OnlyValue' from SingleCell sheet missing"
    );
}

#[test]
fn e2e_xlsx_stress_wide_table() {
    let data = include_bytes!("../corpus/xlsx/multi_sheet_stress.xlsx").to_vec();
    let decoder = XlsxDecoder::new();
    let decode_result = decoder.decode(data).unwrap();
    let result = build_engine().structure(decode_result);

    let full = all_text_of(&result.root);
    assert!(full.contains("Col_01"), "'Col_01' missing from Wide sheet");
    assert!(
        full.contains("Col_20"),
        "'Col_20' missing — wide table truncated?"
    );
    assert!(
        full.contains("Val_20"),
        "'Val_20' missing — wide table data truncated?"
    );
}

#[test]
fn e2e_xlsx_stress_confidence_range() {
    let data = include_bytes!("../corpus/xlsx/multi_sheet_stress.xlsx").to_vec();
    let decoder = XlsxDecoder::new();
    let decode_result = decoder.decode(data).unwrap();
    let result = build_engine().structure(decode_result);

    fn check(node: &olga::model::DocumentNode) {
        assert!(
            (0.0..=1.0).contains(&node.confidence),
            "confidence {} out of range for {:?}",
            node.confidence,
            node.kind
        );
        for child in &node.children {
            check(child);
        }
    }
    check(&result.root);
}
