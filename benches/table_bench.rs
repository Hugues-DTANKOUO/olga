//! Criterion benchmarks for table detection (Brick 7).
//!
//! Measures the 5-step table detection pipeline on synthetic documents:
//!
//! - **lattice_small**: 3×3 table on 1 page (~40 primitives).
//! - **lattice_large**: 20×10 table on 1 page (~300 primitives).
//! - **lattice_multipage**: 50 pages × 5×4 table each (~3000 primitives).
//! - **stream_table**: 10×5 borderless table (~50 text items).
//! - **partial_grid**: 8×3 table with h-lines only (~50 primitives).
//!
//! Target: all benchmarks under 3 seconds for the multipage case.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use olga::model::{BoundingBox, Point, Primitive, PrimitiveKind};
use olga::structure::detectors::table::TableDetector;
use olga::structure::detectors::{DetectionContext, StructureDetector};
use olga::structure::types::FontHistogram;

// ---------------------------------------------------------------------------
// Primitive generators
// ---------------------------------------------------------------------------

fn text_prim(x: f32, y: f32, content: &str, page: u32, order: u64) -> Primitive {
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
            text_direction_origin: olga::model::provenance::ValueOrigin::Observed,
            lang: None,
            lang_origin: None,
        },
        BoundingBox::new(x, y, 0.08, 0.02),
        page,
        order,
    )
}

fn h_line_prim(y: f32, x_start: f32, x_end: f32, page: u32, order: u64) -> Primitive {
    Primitive::observed(
        PrimitiveKind::Line {
            start: Point::new(x_start, y),
            end: Point::new(x_end, y),
            thickness: 0.001,
            color: None,
        },
        BoundingBox::new(x_start, y, x_end - x_start, 0.001),
        page,
        order,
    )
}

fn v_line_prim(x: f32, y_start: f32, y_end: f32, page: u32, order: u64) -> Primitive {
    Primitive::observed(
        PrimitiveKind::Line {
            start: Point::new(x, y_start),
            end: Point::new(x, y_end),
            thickness: 0.001,
            color: None,
        },
        BoundingBox::new(x, y_start, 0.001, y_end - y_start),
        page,
        order,
    )
}

fn ctx() -> DetectionContext {
    DetectionContext {
        font_histogram: FontHistogram::default(),
        page: 0,
        page_dims: None,
        text_boxes: None,
        open_table_columns: None,
    }
}

// ---------------------------------------------------------------------------
// Page generators
// ---------------------------------------------------------------------------

/// Generate a lattice table with `rows` × `cols` cells on a single page.
fn generate_lattice_table(rows: usize, cols: usize) -> Vec<Primitive> {
    let mut prims = Vec::new();
    let mut order = 0u64;

    let x_start = 0.05;
    let y_start = 0.05;
    let col_width = 0.80 / cols as f32;
    let row_height = 0.80 / rows as f32;

    // Text primitives.
    for r in 0..rows {
        for c in 0..cols {
            let x = x_start + c as f32 * col_width + col_width * 0.1;
            let y = y_start + r as f32 * row_height + row_height * 0.3;
            prims.push(text_prim(x, y, &format!("R{}C{}", r, c), 0, order));
            order += 1;
        }
    }

    // Horizontal lines (rows + 1 lines).
    let x_end = x_start + cols as f32 * col_width;
    for r in 0..=rows {
        let y = y_start + r as f32 * row_height;
        prims.push(h_line_prim(y, x_start, x_end, 0, order));
        order += 1;
    }

    // Vertical lines (cols + 1 lines).
    let y_end = y_start + rows as f32 * row_height;
    for c in 0..=cols {
        let x = x_start + c as f32 * col_width;
        prims.push(v_line_prim(x, y_start, y_end, 0, order));
        order += 1;
    }

    prims
}

/// Generate a multipage document: each page has a `rows` × `cols` table.
fn generate_multipage(pages: usize, rows: usize, cols: usize) -> Vec<Vec<Primitive>> {
    (0..pages)
        .map(|_| generate_lattice_table(rows, cols))
        .collect()
}

/// Generate a borderless (stream) table.
fn generate_stream_table(rows: usize, cols: usize) -> Vec<Primitive> {
    let mut prims = Vec::new();
    let mut order = 0u64;

    let x_start = 0.05;
    let y_start = 0.05;
    let col_width = 0.80 / cols as f32;
    let row_height = 0.05;

    for r in 0..rows {
        for c in 0..cols {
            let x = x_start + c as f32 * col_width;
            let y = y_start + r as f32 * row_height;
            prims.push(text_prim(x, y, &format!("R{}C{}", r, c), 0, order));
            order += 1;
        }
    }

    prims
}

/// Generate a partial grid table (h-lines only, no v-lines).
fn generate_partial_grid(rows: usize, cols: usize) -> Vec<Primitive> {
    let mut prims = Vec::new();
    let mut order = 0u64;

    let x_start = 0.05;
    let y_start = 0.05;
    let col_width = 0.80 / cols as f32;
    let row_height = 0.10;

    // Text.
    for r in 0..rows {
        for c in 0..cols {
            let x = x_start + c as f32 * col_width;
            let y = y_start + r as f32 * row_height + row_height * 0.3;
            prims.push(text_prim(x, y, &format!("R{}C{}", r, c), 0, order));
            order += 1;
        }
    }

    // Horizontal lines only.
    let x_end = x_start + cols as f32 * col_width + 0.05;
    for r in 0..=rows {
        let y = y_start + r as f32 * row_height;
        prims.push(h_line_prim(y, x_start, x_end, 0, order));
        order += 1;
    }

    prims
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

fn bench_lattice_small(c: &mut Criterion) {
    c.bench_function("table_lattice_3x3", |b| {
        let page = generate_lattice_table(3, 3);
        let detector = TableDetector::default();
        let context = ctx();
        b.iter(|| {
            let hints = detector.detect(black_box(&page), &context);
            black_box(hints);
        });
    });
}

fn bench_lattice_large(c: &mut Criterion) {
    c.bench_function("table_lattice_20x10", |b| {
        let page = generate_lattice_table(20, 10);
        let detector = TableDetector::default();
        let context = ctx();
        b.iter(|| {
            let hints = detector.detect(black_box(&page), &context);
            black_box(hints);
        });
    });
}

fn bench_lattice_multipage(c: &mut Criterion) {
    c.bench_function("table_lattice_50pages_5x4", |b| {
        let pages = generate_multipage(50, 5, 4);
        let detector = TableDetector::default();
        let context = ctx();
        b.iter(|| {
            let mut total_hints = 0;
            for page in &pages {
                let hints = detector.detect(black_box(page), &context);
                total_hints += hints.len();
            }
            black_box(total_hints);
        });
    });
}

fn bench_stream_table(c: &mut Criterion) {
    c.bench_function("table_stream_10x5", |b| {
        let page = generate_stream_table(10, 5);
        let detector = TableDetector::default();
        let context = ctx();
        b.iter(|| {
            let hints = detector.detect(black_box(&page), &context);
            black_box(hints);
        });
    });
}

fn bench_partial_grid(c: &mut Criterion) {
    c.bench_function("table_partial_grid_8x3", |b| {
        let page = generate_partial_grid(8, 3);
        let detector = TableDetector::default();
        let context = ctx();
        b.iter(|| {
            let hints = detector.detect(black_box(&page), &context);
            black_box(hints);
        });
    });
}

criterion_group!(
    benches,
    bench_lattice_small,
    bench_lattice_large,
    bench_lattice_multipage,
    bench_stream_table,
    bench_partial_grid
);
criterion_main!(benches);
