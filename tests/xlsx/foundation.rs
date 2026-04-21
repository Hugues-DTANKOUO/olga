use olga::formats::xlsx::XlsxDecoder;
use olga::model::*;
use olga::traits::FormatDecoder;

use crate::support::xlsx::{build_xlsx, build_xlsx_with_invalid_shared_strings, col_to_letter};

#[test]
fn decode_simple_spreadsheet() {
    let data = build_xlsx(&[(
        "Sheet1",
        vec![
            vec!["Name", "Age", "City"],
            vec!["Alice", "30", "Paris"],
            vec!["Bob", "25", "London"],
        ],
    )]);

    let decoder = XlsxDecoder::new();
    let result = decoder.decode(data.clone()).unwrap();

    assert_eq!(result.metadata.format, DocumentFormat::Xlsx);
    assert_eq!(result.metadata.page_count, 1);
    // 3 header cells + 6 data cells + 1 sheet title Heading primitive.
    assert_eq!(result.primitives.len(), 10);

    let headers: Vec<_> = result
        .primitives
        .iter()
        .filter(|p| {
            p.hints
                .iter()
                .any(|h| matches!(h.kind, HintKind::TableHeader { .. }))
        })
        .collect();
    assert_eq!(headers.len(), 3);

    let data_cells: Vec<_> = result
        .primitives
        .iter()
        .filter(|p| {
            p.hints
                .iter()
                .any(|h| matches!(h.kind, HintKind::TableCell { .. }))
        })
        .collect();
    assert_eq!(data_cells.len(), 6);

    let sheet_headings: Vec<_> = result
        .primitives
        .iter()
        .filter(|p| {
            p.hints
                .iter()
                .any(|h| matches!(h.kind, HintKind::Heading { level: 1 }))
        })
        .collect();
    assert_eq!(sheet_headings.len(), 1);
    assert!(matches!(
        &sheet_headings[0].kind,
        PrimitiveKind::Text { content, .. } if content == "Sheet1"
    ));

    assert!(result.warnings.iter().any(|warning| {
        warning.kind == olga::error::WarningKind::HeuristicInference
            && warning.message.contains("Sheet 'Sheet1'")
            && warning.message.contains("row 1")
    }));
}

#[test]
fn decode_multi_sheet() {
    let data = build_xlsx(&[
        ("Revenue", vec![vec!["Q1", "Q2"], vec!["100", "200"]]),
        ("Costs", vec![vec!["Q1", "Q2"], vec!["50", "80"]]),
    ]);

    let decoder = XlsxDecoder::new();
    let result = decoder.decode(data.clone()).unwrap();

    assert_eq!(result.metadata.page_count, 2);
    let page0: Vec<_> = result.primitives.iter().filter(|p| p.page == 0).collect();
    let page1: Vec<_> = result.primitives.iter().filter(|p| p.page == 1).collect();
    // 4 cells per sheet + 1 sheet title heading per sheet = 5 each.
    assert_eq!(page0.len(), 5);
    assert_eq!(page1.len(), 5);

    let titles: Vec<&str> = result
        .primitives
        .iter()
        .filter_map(|p| match (&p.kind, p.hints.first()) {
            (PrimitiveKind::Text { content, .. }, Some(hint))
                if matches!(hint.kind, HintKind::Heading { level: 1 }) =>
            {
                Some(content.as_str())
            }
            _ => None,
        })
        .collect();
    assert_eq!(titles, vec!["Revenue", "Costs"]);
}

#[test]
fn decode_metadata() {
    let data = build_xlsx(&[("Sheet1", vec![vec!["A"]]), ("Sheet2", vec![vec!["B"]])]);

    let decoder = XlsxDecoder::new();
    let meta = decoder.metadata(&data).unwrap();
    assert_eq!(meta.format, DocumentFormat::Xlsx);
    assert_eq!(meta.page_count, 2);
    assert!(meta.is_processable());
}

#[test]
fn metadata_uses_raw_workbook_plan_even_with_invalid_shared_strings() {
    let data = build_xlsx_with_invalid_shared_strings();

    let decoder = XlsxDecoder::new();
    let meta = decoder.metadata(&data).unwrap();

    assert_eq!(meta.format, DocumentFormat::Xlsx);
    assert_eq!(meta.page_count, 1);
    assert_eq!(meta.page_count_provenance.origin, ValueOrigin::Observed);
    assert_eq!(
        meta.page_count_provenance.basis,
        PaginationBasis::WorkbookSheets
    );
}

#[test]
fn decode_large_workbook_is_deterministic() {
    let mut owned_rows = Vec::new();
    owned_rows.push(vec![
        "Name".to_string(),
        "Age".to_string(),
        "City".to_string(),
    ]);
    for idx in 0..512 {
        owned_rows.push(vec![
            format!("User {}", idx),
            (20 + (idx % 50)).to_string(),
            format!("City {}", idx % 12),
        ]);
    }

    let borrowed_rows: Vec<Vec<&str>> = owned_rows
        .iter()
        .map(|row| row.iter().map(String::as_str).collect())
        .collect();
    let data = build_xlsx(&[("Large", borrowed_rows)]);

    let decoder = XlsxDecoder::new();
    let first = decoder.decode(data.clone()).unwrap();
    let second = decoder.decode(data.clone()).unwrap();

    assert_eq!(first.primitives.len(), second.primitives.len());
    assert_eq!(
        serde_json::to_string(&first.primitives).unwrap(),
        serde_json::to_string(&second.primitives).unwrap()
    );
    assert_eq!(first.warnings.len(), 1);
    assert_eq!(
        first.warnings[0].kind,
        olga::error::WarningKind::HeuristicInference
    );
    assert_eq!(
        first
            .warnings
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        second
            .warnings
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    );
}

#[test]
fn decode_not_xlsx() {
    let decoder = XlsxDecoder::new();
    assert!(decoder.metadata(b"not xlsx").is_err());
    assert!(decoder.decode(b"not xlsx".to_vec()).is_err());
}

#[test]
fn decode_source_order_monotonic() {
    let data = build_xlsx(&[("Data", vec![vec!["A", "B", "C"], vec!["1", "2", "3"]])]);

    let decoder = XlsxDecoder::new();
    let result = decoder.decode(data.clone()).unwrap();
    for window in result.primitives.windows(2) {
        assert!(window[0].source_order < window[1].source_order);
    }
}

#[test]
fn decode_all_page_zero_single_sheet() {
    let data = build_xlsx(&[("Only", vec![vec!["X", "Y"], vec!["1", "2"]])]);

    let decoder = XlsxDecoder::new();
    let result = decoder.decode(data.clone()).unwrap();
    for prim in &result.primitives {
        assert_eq!(prim.page, 0);
    }
}

#[test]
fn decode_positions_normalized() {
    let data = build_xlsx(&[("Grid", vec![vec!["A", "B"], vec!["C", "D"]])]);

    let decoder = XlsxDecoder::new();
    let result = decoder.decode(data.clone()).unwrap();
    for prim in &result.primitives {
        assert!(prim.bbox.x >= 0.0 && prim.bbox.x <= 1.0);
        assert!(prim.bbox.y >= 0.0 && prim.bbox.y <= 1.0);
    }
}

#[test]
fn col_to_letter_basic() {
    assert_eq!(col_to_letter(0), "A");
    assert_eq!(col_to_letter(1), "B");
    assert_eq!(col_to_letter(25), "Z");
    assert_eq!(col_to_letter(26), "AA");
}
