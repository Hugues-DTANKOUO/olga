use olga::formats::pdf::PdfDecoder;
use olga::model::{
    BoundingBox, DocumentFormat, DocumentMetadata, NodeKind, PaginationBasis, PaginationProvenance,
    Primitive, PrimitiveKind,
};
use olga::traits::FormatDecoder;

use crate::{build_engine, count_nodes};

fn pdf_metadata(page_count: u32) -> DocumentMetadata {
    DocumentMetadata {
        format: DocumentFormat::Pdf,
        page_count,
        page_count_provenance: PaginationProvenance::synthetic(PaginationBasis::FallbackDefault),
        page_dimensions: vec![],
        encrypted: false,
        text_extraction_allowed: true,
        file_size: 0,
        title: None,
    }
}

fn pdf_text(x: f32, y: f32, width: f32, text: &str, page: u32, order: u64) -> Primitive {
    Primitive::observed(
        PrimitiveKind::Text {
            content: text.to_string(),
            font_size: 12.0,
            font_name: "Arial".to_string(),
            is_bold: false,
            is_italic: false,
            color: None,
            char_positions: None,
            text_direction: Default::default(),
            text_direction_origin: olga::model::provenance::ValueOrigin::Observed,
            lang: None,
            lang_origin: None,
        },
        BoundingBox::new(x, y, width, 0.02),
        page,
        order,
    )
}

fn pdf_line(sx: f32, sy: f32, ex: f32, ey: f32, page: u32, order: u64) -> Primitive {
    use olga::model::Point;
    Primitive::observed(
        PrimitiveKind::Line {
            start: Point::new(sx, sy),
            end: Point::new(ex, ey),
            thickness: 0.001,
            color: None,
        },
        BoundingBox::new(
            sx.min(ex),
            sy.min(ey),
            (ex - sx).abs().max(0.001),
            (ey - sy).abs().max(0.001),
        ),
        page,
        order,
    )
}

fn structure_from_primitives(
    prims: Vec<Primitive>,
    page_count: u32,
) -> olga::structure::StructureResult {
    let dr = olga::traits::DecodeResult {
        metadata: pdf_metadata(page_count),
        primitives: prims,
        warnings: vec![],
        images: vec![],
    };
    build_engine().structure(dr)
}

#[test]
fn layout_analyzer_feeds_detectors() {
    let prims = vec![
        pdf_text(0.05, 0.10, 0.40, "First paragraph line.", 0, 0),
        pdf_text(0.05, 0.13, 0.40, "Second paragraph line.", 0, 1),
    ];

    let result = structure_from_primitives(prims, 1);
    let para_count = count_nodes(&result.root, &|k| matches!(k, NodeKind::Paragraph { .. }));
    assert!(
        para_count >= 1,
        "Layout-analyzed primitives should produce paragraph nodes, got {}",
        para_count
    );
}

#[test]
fn layout_list_bullets_detected() {
    let prims = vec![
        pdf_text(0.05, 0.10, 0.40, "• First item", 0, 0),
        pdf_text(0.05, 0.13, 0.40, "• Second item", 0, 1),
        pdf_text(0.05, 0.16, 0.40, "• Third item", 0, 2),
    ];

    let result = structure_from_primitives(prims, 1);
    let list_count = count_nodes(&result.root, &|k| matches!(k, NodeKind::List { .. }));
    let item_count = count_nodes(&result.root, &|k| matches!(k, NodeKind::ListItem { .. }));

    assert!(
        list_count >= 1,
        "Should detect at least 1 list, got {}",
        list_count
    );
    assert!(
        item_count >= 3,
        "Should detect at least 3 list items, got {}",
        item_count
    );
}

#[test]
fn layout_list_numbered_detected() {
    let prims = vec![
        pdf_text(0.05, 0.10, 0.40, "1. Step one", 0, 0),
        pdf_text(0.05, 0.13, 0.40, "2. Step two", 0, 1),
        pdf_text(0.05, 0.16, 0.40, "3. Step three", 0, 2),
    ];

    let result = structure_from_primitives(prims, 1);
    let list_count = count_nodes(&result.root, &|k| matches!(k, NodeKind::List { .. }));

    assert!(
        list_count >= 1,
        "Should detect at least 1 ordered list, got {}",
        list_count
    );
}

#[test]
fn layout_table_with_rules_detected() {
    let mut prims = vec![
        pdf_text(0.10, 0.12, 0.15, "Cell A1", 0, 0),
        pdf_text(0.30, 0.12, 0.15, "Cell B1", 0, 1),
        pdf_text(0.10, 0.22, 0.15, "Cell A2", 0, 2),
        pdf_text(0.30, 0.22, 0.15, "Cell B2", 0, 3),
    ];
    prims.push(pdf_line(0.05, 0.10, 0.50, 0.10, 0, 4));
    prims.push(pdf_line(0.05, 0.17, 0.50, 0.17, 0, 5));
    prims.push(pdf_line(0.05, 0.27, 0.50, 0.27, 0, 6));
    prims.push(pdf_line(0.05, 0.10, 0.05, 0.27, 0, 7));
    prims.push(pdf_line(0.25, 0.10, 0.25, 0.27, 0, 8));
    prims.push(pdf_line(0.50, 0.10, 0.50, 0.27, 0, 9));

    let result = structure_from_primitives(prims, 1);
    let table_count = count_nodes(&result.root, &|k| matches!(k, NodeKind::Table { .. }));

    assert!(
        table_count >= 1,
        "Should detect at least 1 ruled table, got {}",
        table_count
    );
}

#[test]
fn layout_table_borderless_detected() {
    let prims = vec![
        pdf_text(0.05, 0.10, 0.15, "Name", 0, 0),
        pdf_text(0.40, 0.10, 0.15, "Age", 0, 1),
        pdf_text(0.05, 0.15, 0.15, "Alice", 0, 2),
        pdf_text(0.40, 0.15, 0.15, "30", 0, 3),
        pdf_text(0.05, 0.20, 0.15, "Bob", 0, 4),
        pdf_text(0.40, 0.20, 0.15, "25", 0, 5),
    ];

    let result = structure_from_primitives(prims, 1);
    let table_count = count_nodes(&result.root, &|k| matches!(k, NodeKind::Table { .. }));

    assert!(
        table_count >= 1,
        "Should detect at least 1 borderless table, got {}",
        table_count
    );
}

#[test]
fn layout_mixed_content_page() {
    let prims = vec![
        Primitive::observed(
            PrimitiveKind::Text {
                content: "Important Report".to_string(),
                font_size: 24.0,
                font_name: "Arial".to_string(),
                is_bold: true,
                is_italic: false,
                color: None,
                char_positions: None,
                text_direction: Default::default(),
                text_direction_origin: olga::model::provenance::ValueOrigin::Observed,
                lang: None,
                lang_origin: None,
            },
            BoundingBox::new(0.05, 0.05, 0.50, 0.04),
            0,
            0,
        ),
        pdf_text(
            0.05,
            0.12,
            0.80,
            "This report covers quarterly results.",
            0,
            1,
        ),
        pdf_text(
            0.05,
            0.15,
            0.80,
            "Performance was strong across all segments.",
            0,
            2,
        ),
        pdf_text(0.05, 0.25, 0.40, "• Revenue up 15%", 0, 3),
        pdf_text(0.05, 0.28, 0.40, "• Costs down 8%", 0, 4),
        pdf_text(0.05, 0.31, 0.40, "• Profit margin improved", 0, 5),
    ];

    let result = structure_from_primitives(prims, 1);

    let total = result.root.node_count();
    assert!(
        total >= 5,
        "Mixed content page should produce at least 5 nodes, got {}",
        total
    );

    let has_heading = count_nodes(&result.root, &|k| matches!(k, NodeKind::Heading { .. })) > 0;
    let has_para = count_nodes(&result.root, &|k| matches!(k, NodeKind::Paragraph { .. })) > 0;
    let has_list = count_nodes(&result.root, &|k| matches!(k, NodeKind::List { .. })) > 0;

    assert!(
        has_heading || has_para,
        "Should detect heading or paragraph"
    );
    assert!(has_list, "Should detect at least one list");
}

#[test]
fn layout_two_column_reading_order() {
    let prims = vec![
        pdf_text(0.05, 0.10, 0.35, "Left col line 1", 0, 0),
        pdf_text(0.05, 0.13, 0.35, "Left col line 2", 0, 1),
        pdf_text(0.55, 0.10, 0.35, "Right col line 1", 0, 2),
        pdf_text(0.55, 0.13, 0.35, "Right col line 2", 0, 3),
    ];

    let result = structure_from_primitives(prims, 1);

    let all_text = result.root.all_text();
    let left_pos = all_text.find("Left col").unwrap_or(usize::MAX);
    let right_pos = all_text.find("Right col").unwrap_or(usize::MAX);

    assert!(
        left_pos < right_pos,
        "Left column should come before right column in reading order.\nFull text: {}",
        all_text
    );
}

#[test]
fn layout_table_partial_grid() {
    let mut prims = vec![
        pdf_text(0.10, 0.12, 0.15, "Name", 0, 0),
        pdf_text(0.35, 0.12, 0.15, "Score", 0, 1),
        pdf_text(0.10, 0.22, 0.15, "Alice", 0, 2),
        pdf_text(0.35, 0.22, 0.15, "95", 0, 3),
    ];
    prims.push(pdf_line(0.05, 0.10, 0.55, 0.10, 0, 4));
    prims.push(pdf_line(0.05, 0.17, 0.55, 0.17, 0, 5));
    prims.push(pdf_line(0.05, 0.27, 0.55, 0.27, 0, 6));

    let result = structure_from_primitives(prims, 1);
    let table_count = count_nodes(&result.root, &|k| matches!(k, NodeKind::Table { .. }));

    assert!(
        table_count >= 1,
        "Should detect a partial-grid table (h-rules only), got {}",
        table_count
    );
}

#[test]
fn corpus_pdf_multi_page_no_regression() {
    let pdf_path = "tests/corpus/pdf/multi_page_stress.pdf";
    let pdf_data =
        std::fs::read(pdf_path).expect("Failed to read multi_page_stress.pdf from corpus");

    let decoder = PdfDecoder;
    let decode_result = decoder
        .decode(pdf_data)
        .expect("Failed to decode multi_page_stress.pdf");

    let structure_result = build_engine().structure(decode_result);

    let node_count = structure_result.root.node_count();
    assert!(
        node_count >= 5,
        "Multi-page PDF should produce at least 5 nodes, got {}",
        node_count
    );
}
