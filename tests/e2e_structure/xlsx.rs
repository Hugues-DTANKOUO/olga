use olga::formats::xlsx::XlsxDecoder;
use olga::model::{NodeKind, StructureSource};
use olga::traits::FormatDecoder;

use crate::{build_engine, collect_nodes_by_kind};

#[test]
fn e2e_xlsx_employee_directory_decodes_and_structures() {
    let data = include_bytes!("../corpus/xlsx/employee_directory.xlsx").to_vec();
    let decoder = XlsxDecoder::new();
    let decode_result = decoder.decode(data).expect("XLSX decode should succeed");

    assert!(
        !decode_result.primitives.is_empty(),
        "should extract primitives from XLSX"
    );

    let engine = build_engine();
    let result = engine.structure(decode_result);

    assert!(matches!(result.root.kind, NodeKind::Document));
    assert!(
        !result.root.children.is_empty(),
        "XLSX document should have children"
    );
}

#[test]
fn e2e_xlsx_employee_directory_has_table() {
    let data = include_bytes!("../corpus/xlsx/employee_directory.xlsx").to_vec();
    let decoder = XlsxDecoder::new();
    let decode_result = decoder.decode(data).expect("XLSX decode should succeed");
    let engine = build_engine();
    let result = engine.structure(decode_result);

    let tables = collect_nodes_by_kind(&result.root, &|k| matches!(k, NodeKind::Table { .. }));
    assert!(
        !tables.is_empty(),
        "XLSX should produce at least one Table node"
    );

    let cells = collect_nodes_by_kind(&result.root, &|k| matches!(k, NodeKind::TableCell { .. }));
    assert!(
        cells.len() >= 20,
        "expected at least 20 table cells, got {}",
        cells.len()
    );

    let cell_texts: Vec<&str> = cells.iter().filter_map(|n| n.text()).collect();
    assert!(
        cell_texts.iter().any(|t| t.contains("Alice")),
        "should contain 'Alice' in cells, got {} cells",
        cell_texts.len()
    );
    assert!(
        cell_texts.iter().any(|t| t.contains("Engineering")),
        "should contain 'Engineering' in cells"
    );
}

#[test]
fn e2e_xlsx_employee_directory_provenance_is_format_derived() {
    let data = include_bytes!("../corpus/xlsx/employee_directory.xlsx").to_vec();
    let decoder = XlsxDecoder::new();
    let decode_result = decoder.decode(data).expect("XLSX decode should succeed");
    let engine = build_engine();
    let result = engine.structure(decode_result);

    let tables = collect_nodes_by_kind(&result.root, &|k| matches!(k, NodeKind::Table { .. }));
    for table in &tables {
        assert!(
            matches!(table.structure_source, StructureSource::HintAssembled),
            "XLSX tables should be HintAssembled, got {:?}",
            table.structure_source
        );
    }
}
