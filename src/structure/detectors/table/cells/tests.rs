use super::super::edges::{CellValidation, Edge, Orientation};
use super::super::intersections::Intersection;
use super::*;
use crate::model::{BoundingBox, Primitive, PrimitiveKind};

fn make_grid_edges(xs: &[f32], ys: &[f32], x_range: (f32, f32), y_range: (f32, f32)) -> Vec<Edge> {
    let mut edges = Vec::new();
    for &y in ys {
        edges.push(Edge {
            orientation: Orientation::Horizontal,
            position: y,
            start: x_range.0,
            end: x_range.1,
        });
    }
    for &x in xs {
        edges.push(Edge {
            orientation: Orientation::Vertical,
            position: x,
            start: y_range.0,
            end: y_range.1,
        });
    }
    edges
}

fn make_grid_intersections(xs: &[f32], ys: &[f32]) -> Vec<Intersection> {
    let mut ints = Vec::new();
    for &y in ys {
        for &x in xs {
            ints.push(Intersection { x, y });
        }
    }
    ints
}

fn text_prim(x: f32, y: f32, content: &str) -> Primitive {
    Primitive::observed(
        PrimitiveKind::Text {
            content: content.to_string(),
            font_size: 12.0,
            font_name: "Arial".to_string(),
            is_bold: false,
            is_italic: false,
            color: None,
            char_positions: None,
            text_direction: Default::default(),
            text_direction_origin: crate::model::provenance::ValueOrigin::Observed,
            lang: None,
            lang_origin: None,
        },
        BoundingBox::new(x, y, 0.08, 0.02),
        0,
        0,
    )
}

#[test]
fn construct_2x2_grid() {
    let xs = [0.1, 0.5, 0.9];
    let ys = [0.1, 0.4, 0.7];
    let edges = make_grid_edges(&xs, &ys, (0.1, 0.9), (0.1, 0.7));
    let intersections = make_grid_intersections(&xs, &ys);

    let cells = construct_cells(&intersections, &edges, 0.005, CellValidation::Relaxed);
    assert_eq!(cells.len(), 4, "3×3 grid → 4 cells");
}

#[test]
fn construct_3x3_grid() {
    let xs = [0.1, 0.3, 0.6, 0.9];
    let ys = [0.1, 0.3, 0.5, 0.8];
    let edges = make_grid_edges(&xs, &ys, (0.1, 0.9), (0.1, 0.8));
    let intersections = make_grid_intersections(&xs, &ys);

    let cells = construct_cells(&intersections, &edges, 0.005, CellValidation::Relaxed);
    assert_eq!(cells.len(), 9, "4×4 grid → 9 cells");
}

#[test]
fn no_cells_from_too_few_intersections() {
    let intersections = vec![
        Intersection { x: 0.1, y: 0.1 },
        Intersection { x: 0.5, y: 0.1 },
    ];
    let edges = vec![Edge {
        orientation: Orientation::Horizontal,
        position: 0.1,
        start: 0.1,
        end: 0.5,
    }];
    let cells = construct_cells(&intersections, &edges, 0.005, CellValidation::Relaxed);
    assert!(cells.is_empty());
}

#[test]
fn group_two_separate_tables() {
    let xs1 = [0.05, 0.2, 0.4];
    let ys1 = [0.05, 0.15, 0.25];
    let edges1 = make_grid_edges(&xs1, &ys1, (0.05, 0.4), (0.05, 0.25));
    let ints1 = make_grid_intersections(&xs1, &ys1);

    let xs2 = [0.55, 0.7, 0.9];
    let ys2 = [0.55, 0.7, 0.85];
    let edges2 = make_grid_edges(&xs2, &ys2, (0.55, 0.9), (0.55, 0.85));
    let ints2 = make_grid_intersections(&xs2, &ys2);

    let mut all_edges = edges1;
    all_edges.extend(edges2);
    let mut all_ints = ints1;
    all_ints.extend(ints2);

    let cells = construct_cells(&all_ints, &all_edges, 0.005, CellValidation::Relaxed);
    assert_eq!(cells.len(), 8, "Two 2×2 grids → 8 cells");

    let page = vec![
        text_prim(0.10, 0.10, "T1-A"),
        text_prim(0.25, 0.10, "T1-B"),
        text_prim(0.60, 0.60, "T2-A"),
        text_prim(0.75, 0.60, "T2-B"),
    ];

    let tables = group_into_tables(&cells, &page, 0.005, 2, 2);
    assert_eq!(tables.len(), 2, "Should group into 2 separate tables");
}

#[test]
fn table_without_text_is_filtered() {
    let xs = [0.1, 0.5, 0.9];
    let ys = [0.1, 0.4, 0.7];
    let edges = make_grid_edges(&xs, &ys, (0.1, 0.9), (0.1, 0.7));
    let intersections = make_grid_intersections(&xs, &ys);
    let cells = construct_cells(&intersections, &edges, 0.005, CellValidation::Relaxed);

    let page: Vec<Primitive> = Vec::new();
    let tables = group_into_tables(&cells, &page, 0.005, 2, 2);
    assert!(tables.is_empty(), "Table without text should be filtered");
}

#[test]
fn assign_text_to_cells() {
    let xs = [0.1, 0.5, 0.9];
    let ys = [0.1, 0.4, 0.7];

    let table = Table {
        cells: Vec::new(),
        row_ys: ys.to_vec(),
        col_xs: xs.to_vec(),
    };

    let page = vec![
        text_prim(0.20, 0.20, "Cell(0,0)"),
        text_prim(0.60, 0.20, "Cell(0,1)"),
        text_prim(0.20, 0.50, "Cell(1,0)"),
        text_prim(0.60, 0.50, "Cell(1,1)"),
    ];

    let assignments = assign_text_to_table(&table, &page);
    assert_eq!(assignments.len(), 4);
    assert!(assignments.contains(&(0, 0, 0)));
    assert!(assignments.contains(&(1, 0, 1)));
    assert!(assignments.contains(&(2, 1, 0)));
    assert!(assignments.contains(&(3, 1, 1)));
}

#[test]
fn corner_indexed_grouping_detects_adjacency() {
    let xs = [0.1, 0.5, 0.9];
    let ys = [0.1, 0.4, 0.7];
    let edges = make_grid_edges(&xs, &ys, (0.1, 0.9), (0.1, 0.7));
    let intersections = make_grid_intersections(&xs, &ys);

    let cells = construct_cells(&intersections, &edges, 0.005, CellValidation::Relaxed);
    assert_eq!(cells.len(), 4);

    let page = vec![
        text_prim(0.20, 0.20, "A"),
        text_prim(0.60, 0.20, "B"),
        text_prim(0.20, 0.50, "C"),
        text_prim(0.60, 0.50, "D"),
    ];
    let tables = group_into_tables(&cells, &page, 0.005, 2, 2);
    assert_eq!(
        tables.len(),
        1,
        "Adjacent cells should group into one table"
    );
}

#[test]
fn strict_validation_requires_all_four_edges() {
    let xs = [0.1, 0.5];
    let ys = [0.1, 0.4];
    let intersections = make_grid_intersections(&xs, &ys);
    let edges = vec![
        Edge {
            orientation: Orientation::Horizontal,
            position: 0.1,
            start: 0.1,
            end: 0.5,
        },
        Edge {
            orientation: Orientation::Horizontal,
            position: 0.4,
            start: 0.1,
            end: 0.5,
        },
    ];

    let relaxed = construct_cells(&intersections, &edges, 0.005, CellValidation::Relaxed);
    assert_eq!(relaxed.len(), 1, "Relaxed mode should find the cell");

    let strict = construct_cells(&intersections, &edges, 0.005, CellValidation::Strict);
    assert!(strict.is_empty(), "Strict mode should reject partial cell");
}

#[test]
fn unique_positions_clusters() {
    let positions = vec![0.100, 0.102, 0.103, 0.500, 0.501, 0.900];
    let unique = super::spatial::unique_positions(positions.into_iter(), 0.005);
    assert_eq!(unique.len(), 3, "Should cluster to 3 positions");
}

fn text_prim_with_chars(
    x: f32,
    y: f32,
    content: &str,
    chars: Vec<crate::model::CharPosition>,
) -> Primitive {
    Primitive::observed(
        PrimitiveKind::Text {
            content: content.to_string(),
            font_size: 12.0,
            font_name: "Arial".to_string(),
            is_bold: false,
            is_italic: false,
            color: None,
            char_positions: Some(chars),
            text_direction: Default::default(),
            text_direction_origin: crate::model::provenance::ValueOrigin::Observed,
            lang: None,
            lang_origin: None,
        },
        BoundingBox::new(x, y, 0.08, 0.02),
        0,
        0,
    )
}

#[test]
fn char_level_assignment_majority_vote() {
    let table = Table {
        cells: Vec::new(),
        row_ys: vec![0.1, 0.4],
        col_xs: vec![0.1, 0.5, 0.9],
    };

    let chars = vec![
        crate::model::CharPosition {
            ch: 'A',
            bbox: BoundingBox::new(0.45, 0.2, 0.06, 0.02),
        },
        crate::model::CharPosition {
            ch: 'B',
            bbox: BoundingBox::new(0.51, 0.2, 0.04, 0.02),
        },
        crate::model::CharPosition {
            ch: 'C',
            bbox: BoundingBox::new(0.55, 0.2, 0.04, 0.02),
        },
        crate::model::CharPosition {
            ch: 'D',
            bbox: BoundingBox::new(0.59, 0.2, 0.04, 0.02),
        },
    ];

    let prim = text_prim_with_chars(0.45, 0.2, "ABCD", chars);
    let page = vec![prim];

    let assignments = assign_text_to_table(&table, &page);
    assert_eq!(assignments.len(), 1);
    assert_eq!(assignments[0], (0, 0, 1));
}

#[test]
fn char_level_falls_back_when_no_char_positions() {
    let table = Table {
        cells: Vec::new(),
        row_ys: vec![0.1, 0.4],
        col_xs: vec![0.1, 0.5, 0.9],
    };

    let prim = text_prim(0.45, 0.2, "ABCD");
    let page = vec![prim];

    let assignments = assign_text_to_table(&table, &page);
    assert_eq!(assignments.len(), 1);
    assert_eq!(assignments[0], (0, 0, 0));
}
