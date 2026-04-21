use olga::formats::xlsx::XlsxDecoder;
use olga::model::NodeKind;
use olga::traits::FormatDecoder;

use crate::support::xlsx::build_xlsx;
use crate::{build_engine, collect_nodes};

#[test]
fn stress_xlsx_large_table() {
    let mut rows = vec![
        vec![
            "ID", "Name", "Dept", "Role", "Level", "Salary", "Start", "End", "Active", "Score",
        ]
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>(),
    ];

    for i in 1..=50 {
        rows.push(vec![
            format!("{}", i),
            format!("Employee_{}", i),
            format!("Dept_{}", i % 5),
            format!("Role_{}", i % 3),
            format!("{}", (i % 5) + 1),
            format!("{}", 50000 + i * 1000),
            "2020-01-15".to_string(),
            "2025-12-31".to_string(),
            if i % 7 == 0 {
                "No".to_string()
            } else {
                "Yes".to_string()
            },
            format!("{:.1}", 3.0 + (i as f64 % 20.0) / 10.0),
        ]);
    }

    let sheet_data: Vec<Vec<&str>> = rows
        .iter()
        .map(|row| row.iter().map(|s| s.as_str()).collect())
        .collect();
    let sheets = [("Employees", sheet_data)];
    let xlsx = build_xlsx(
        &sheets
            .iter()
            .map(|(name, rows)| {
                (
                    *name,
                    rows.iter().map(|r| r.to_vec()).collect::<Vec<Vec<&str>>>(),
                )
            })
            .collect::<Vec<_>>(),
    );

    let decoder = XlsxDecoder::new();
    let dr = decoder.decode(xlsx).expect("XLSX decode failed");
    let result = build_engine().structure(dr);

    let cells = collect_nodes(&result.root, &|k| matches!(k, NodeKind::TableCell { .. }));
    assert!(
        cells.len() >= 400,
        "expected at least 400 cells from 50-row table, got {}",
        cells.len()
    );

    let full = result.root.all_text();
    assert!(full.contains("Employee_1"), "'Employee_1' missing");
    assert!(full.contains("Employee_50"), "'Employee_50' missing");
}

#[test]
fn stress_xlsx_multi_sheet() {
    let sheets: Vec<(&str, Vec<Vec<&str>>)> = vec![
        (
            "Revenue",
            vec![
                vec!["Quarter", "Amount"],
                vec!["Q1", "100000"],
                vec!["Q2", "120000"],
            ],
        ),
        (
            "Costs",
            vec![
                vec!["Category", "Amount"],
                vec!["Salaries", "80000"],
                vec!["Rent", "15000"],
            ],
        ),
        (
            "Summary",
            vec![vec!["Metric", "Value"], vec!["Profit", "125000"]],
        ),
    ];

    let xlsx = build_xlsx(&sheets);
    let decoder = XlsxDecoder::new();
    let dr = decoder.decode(xlsx).expect("XLSX decode failed");
    let result = build_engine().structure(dr);

    let full = result.root.all_text();
    assert!(full.contains("Q1"), "'Q1' from Revenue missing");
    assert!(full.contains("Salaries"), "'Salaries' from Costs missing");
    assert!(full.contains("Profit"), "'Profit' from Summary missing");

    let tables = collect_nodes(&result.root, &|k| matches!(k, NodeKind::Table { .. }));
    assert!(
        tables.len() >= 3,
        "3 sheets should produce at least 3 Table nodes, got {}",
        tables.len()
    );
}

#[test]
fn stress_xlsx_single_cell() {
    let sheets: Vec<(&str, Vec<Vec<&str>>)> = vec![("Minimal", vec![vec!["OnlyCell"]])];
    let xlsx = build_xlsx(&sheets);
    let decoder = XlsxDecoder::new();
    let dr = decoder.decode(xlsx).expect("XLSX decode failed");
    let result = build_engine().structure(dr);

    assert!(matches!(result.root.kind, NodeKind::Document));
    let full = result.root.all_text();
    assert!(full.contains("OnlyCell"), "'OnlyCell' missing");
}
