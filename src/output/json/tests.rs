use super::*;
use crate::error::Warning;
use crate::model::provenance::StructureSource;
use crate::model::{BoundingBox, DocumentFormat, DocumentMetadata, DocumentNode, NodeKind};
use crate::model::{PaginationBasis, PaginationProvenance};
use crate::structure::StructureResult;

fn test_metadata() -> DocumentMetadata {
    DocumentMetadata {
        format: DocumentFormat::Pdf,
        page_count: 1,
        page_count_provenance: PaginationProvenance::observed(PaginationBasis::PhysicalPages),
        page_dimensions: vec![],
        encrypted: false,
        text_extraction_allowed: true,
        file_size: 1024,
        title: Some("Test Document".to_string()),
    }
}

#[test]
fn json_renders_simple_document() {
    let root = DocumentNode {
        kind: NodeKind::Document,
        bbox: BoundingBox::new(0.0, 0.0, 1.0, 1.0),
        confidence: 1.0,
        structure_source: StructureSource::HintAssembled,
        source_pages: 0..1,
        children: vec![DocumentNode {
            kind: NodeKind::Heading {
                level: 1,
                text: "Hello World".to_string(),
            },
            bbox: BoundingBox::new(0.1, 0.05, 0.4, 0.03),
            confidence: 1.0,
            structure_source: StructureSource::HintAssembled,
            source_pages: 0..1,
            children: vec![],
            metadata: Default::default(),
        }],
        metadata: Default::default(),
    };

    let result = StructureResult {
        metadata: test_metadata(),
        root,
        artifacts: vec![],
        warnings: vec![],
        heuristic_pages: vec![],
    };

    let json = render(&result);
    assert_eq!(json["olga_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(json["source"]["format"], "PDF");
    assert_eq!(json["source"]["title"], "Test Document");
    assert_eq!(json["elements"][0]["type"], "heading");
    assert_eq!(json["elements"][0]["text"], "Hello World");
    assert_eq!(json["elements"][0]["id"], "e-0-0");
    // confidence=1.0 and source="format-derived" are stripped as defaults.
    assert!(json["elements"][0].get("confidence").is_none());
    assert!(json["elements"][0].get("source").is_none());
}

#[test]
fn json_renders_table_with_dual_representation() {
    let cells = [("Name", 0, 0), ("Age", 0, 1), ("Alice", 1, 0), ("30", 1, 1)];

    let mut table_node = DocumentNode {
        kind: NodeKind::Table { rows: 2, cols: 2 },
        bbox: BoundingBox::new(0.1, 0.2, 0.8, 0.2),
        confidence: 0.95,
        structure_source: StructureSource::HeuristicDetected {
            detector: "TableDetector".to_string(),
            confidence: 0.95,
        },
        source_pages: 0..1,
        children: vec![],
        metadata: Default::default(),
    };

    // Add rows with cells
    for row in 0..2u32 {
        let mut row_node = DocumentNode {
            kind: NodeKind::TableRow,
            bbox: BoundingBox::new(0.1, 0.2 + row as f32 * 0.1, 0.8, 0.1),
            confidence: 0.95,
            structure_source: StructureSource::HeuristicDetected {
                detector: "TableDetector".to_string(),
                confidence: 0.95,
            },
            source_pages: 0..1,
            children: vec![],
            metadata: Default::default(),
        };

        for &(text, r, c) in cells.iter().filter(|&&(_, r, _)| r == row) {
            let cell_node = DocumentNode {
                kind: NodeKind::TableCell {
                    row: r,
                    col: c,
                    rowspan: 1,
                    colspan: 1,
                    text: text.to_string(),
                },
                bbox: BoundingBox::new(0.1 + c as f32 * 0.4, 0.2 + r as f32 * 0.1, 0.4, 0.1),
                confidence: 0.95,
                structure_source: StructureSource::HeuristicDetected {
                    detector: "TableDetector".to_string(),
                    confidence: 0.95,
                },
                source_pages: 0..1,
                children: vec![],
                metadata: Default::default(),
            };
            row_node.children.push(cell_node);
        }
        table_node.children.push(row_node);
    }

    let root = DocumentNode {
        kind: NodeKind::Document,
        bbox: BoundingBox::new(0.0, 0.0, 1.0, 1.0),
        confidence: 1.0,
        structure_source: StructureSource::HintAssembled,
        source_pages: 0..1,
        children: vec![table_node],
        metadata: Default::default(),
    };

    let result = StructureResult {
        metadata: test_metadata(),
        root,
        artifacts: vec![],
        warnings: vec![],
        heuristic_pages: vec![0],
    };

    let json = render(&result);
    let table = &json["elements"][0];

    // Simple representation
    assert_eq!(table["type"], "table");
    assert_eq!(table["headers"], json!(["Name", "Age"]));
    assert_eq!(table["data"], json!([["Alice", "30"]]));
    assert_eq!(table["source"], "heuristic:TableDetector");

    // Detailed cells array is omitted for simple tables (all rowspan/colspan = 1).
    assert!(table.get("cells").is_none());
}

#[test]
fn json_includes_warnings() {
    let root = DocumentNode {
        kind: NodeKind::Document,
        bbox: BoundingBox::new(0.0, 0.0, 1.0, 1.0),
        confidence: 1.0,
        structure_source: StructureSource::HintAssembled,
        source_pages: 0..1,
        children: vec![],
        metadata: Default::default(),
    };

    let result = StructureResult {
        metadata: test_metadata(),
        root,
        artifacts: vec![],
        warnings: vec![Warning {
            kind: crate::error::WarningKind::HeuristicInference,
            message: "Font histogram sparse".to_string(),
            page: Some(0),
        }],
        heuristic_pages: vec![],
    };

    let json = render(&result);
    assert!(json["warnings"].is_array());
    assert_eq!(json["warnings"][0]["message"], "Font histogram sparse");
}
