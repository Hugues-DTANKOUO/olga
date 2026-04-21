use super::*;
use crate::model::hints::SemanticHint;
use crate::model::provenance::GeometrySpace;
use crate::model::{BoundingBox, DocumentMetadata, HintKind, Primitive, PrimitiveKind};

fn text_prim(content: &str, page: u32, order: u64) -> Primitive {
    Primitive::reconstructed(
        PrimitiveKind::Text {
            content: content.to_string(),
            font_size: 12.0,
            font_name: "Arial".to_string(),
            is_bold: false,
            is_italic: false,
            color: None,
            char_positions: None,
            text_direction: crate::model::TextDirection::LeftToRight,
            text_direction_origin: crate::model::provenance::ValueOrigin::Reconstructed,
            lang: None,
            lang_origin: None,
        },
        BoundingBox::new(0.0, 0.0, 0.5, 0.015),
        page,
        order,
        GeometrySpace::LogicalPage,
    )
}

fn text_prim_bold(content: &str, page: u32, order: u64) -> Primitive {
    let mut p = text_prim(content, page, order);
    if let PrimitiveKind::Text {
        ref mut is_bold, ..
    } = p.kind
    {
        *is_bold = true;
    }
    p
}

fn text_prim_with_hint(content: &str, page: u32, order: u64, hint: HintKind) -> Primitive {
    let mut p = text_prim(content, page, order);
    p.hints.push(SemanticHint::from_format(hint));
    p
}

fn test_metadata(page_count: u32) -> DocumentMetadata {
    use crate::model::{DocumentFormat, PaginationBasis, PaginationProvenance};
    DocumentMetadata {
        format: DocumentFormat::Docx,
        page_count,
        page_count_provenance: PaginationProvenance::observed(PaginationBasis::PhysicalPages),
        page_dimensions: vec![],
        encrypted: false,
        text_extraction_allowed: true,
        file_size: 1024,
        title: None,
    }
}

#[test]
fn sequential_renders_paragraphs_in_order() {
    let prims = vec![
        text_prim("First paragraph.", 0, 0),
        text_prim("Second paragraph.", 0, 1),
    ];
    let meta = test_metadata(1);
    let result = render_text(&prims, &meta);
    let first_pos = result.find("First").unwrap();
    let second_pos = result.find("Second").unwrap();
    assert!(first_pos < second_pos);
}

#[test]
fn markdown_renders_heading_with_hashes() {
    let prims = vec![text_prim_with_hint(
        "My Heading",
        0,
        0,
        HintKind::Heading { level: 2 },
    )];
    let meta = test_metadata(1);
    let result = render_markdown(&prims, &meta);
    assert!(
        result.contains("## My Heading"),
        "expected '## My Heading', got: {}",
        result
    );
}

#[test]
fn markdown_renders_bold_text() {
    let prims = vec![text_prim_bold("Important", 0, 0)];
    let meta = test_metadata(1);
    let result = render_markdown(&prims, &meta);
    assert!(
        result.contains("**Important**"),
        "expected bold markers, got: {}",
        result
    );
}

#[test]
fn markdown_renders_table_cells_as_gfm() {
    let prims = vec![
        text_prim_with_hint(
            "Name",
            0,
            0,
            HintKind::TableCell {
                row: 0,
                col: 0,
                rowspan: 1,
                colspan: 1,
            },
        ),
        text_prim_with_hint(
            "Age",
            0,
            1,
            HintKind::TableCell {
                row: 0,
                col: 1,
                rowspan: 1,
                colspan: 1,
            },
        ),
        text_prim_with_hint(
            "Alice",
            0,
            2,
            HintKind::TableCell {
                row: 1,
                col: 0,
                rowspan: 1,
                colspan: 1,
            },
        ),
        text_prim_with_hint(
            "30",
            0,
            3,
            HintKind::TableCell {
                row: 1,
                col: 1,
                rowspan: 1,
                colspan: 1,
            },
        ),
    ];
    let meta = test_metadata(1);
    let result = render_markdown(&prims, &meta);
    assert!(
        result.contains("| Name"),
        "expected GFM table, got: {}",
        result
    );
    assert!(
        result.contains("| ---"),
        "expected separator, got: {}",
        result
    );
    assert!(
        result.contains("| Alice"),
        "expected data row, got: {}",
        result
    );
}

#[test]
fn multipage_renders_page_separators() {
    let prims = vec![
        text_prim("Page one content", 0, 0),
        text_prim("Page two content", 1, 0),
    ];
    let meta = test_metadata(2);
    let result = render_text(&prims, &meta);
    assert!(
        result.contains("--- page 2 ---"),
        "expected page separator, got: {}",
        result
    );
}

#[test]
fn text_renders_table_as_ascii() {
    let prims = vec![
        text_prim_with_hint(
            "Col A",
            0,
            0,
            HintKind::TableCell {
                row: 0,
                col: 0,
                rowspan: 1,
                colspan: 1,
            },
        ),
        text_prim_with_hint(
            "Col B",
            0,
            1,
            HintKind::TableCell {
                row: 0,
                col: 1,
                rowspan: 1,
                colspan: 1,
            },
        ),
        text_prim_with_hint(
            "val1",
            0,
            2,
            HintKind::TableCell {
                row: 1,
                col: 0,
                rowspan: 1,
                colspan: 1,
            },
        ),
        text_prim_with_hint(
            "val2",
            0,
            3,
            HintKind::TableCell {
                row: 1,
                col: 1,
                rowspan: 1,
                colspan: 1,
            },
        ),
    ];
    let meta = test_metadata(1);
    let result = render_text(&prims, &meta);
    assert!(result.contains("Col A"), "got: {}", result);
    assert!(
        result.contains("---"),
        "expected separator, got: {}",
        result
    );
}

#[test]
fn list_items_get_prefixed() {
    let prims = vec![
        text_prim_with_hint(
            "First item",
            0,
            0,
            HintKind::ListItem {
                depth: 0,
                ordered: false,
                list_group: None,
            },
        ),
        text_prim_with_hint(
            "Second item",
            0,
            1,
            HintKind::ListItem {
                depth: 0,
                ordered: false,
                list_group: None,
            },
        ),
    ];
    let meta = test_metadata(1);
    let result = render_markdown(&prims, &meta);
    assert!(result.contains("- First item"), "got: {}", result);
    assert!(result.contains("- Second item"), "got: {}", result);
}
