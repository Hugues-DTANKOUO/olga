use super::*;

/// Build a fully ruled NxM lattice page (H+V lines + text in each cell).
fn build_lattice_page(rows: usize, cols: usize) -> Vec<Primitive> {
    let mut page = Vec::new();
    let mut order = 0u64;

    let x_start = 0.05;
    let y_start = 0.05;
    let col_width = 0.85 / cols as f32;
    let row_height = 0.85 / rows as f32;

    // Horizontal lines (rows + 1).
    for r in 0..=rows {
        let y = y_start + r as f32 * row_height;
        page.push(h_line(y, x_start, x_start + cols as f32 * col_width, order));
        order += 1;
    }
    // Vertical lines (cols + 1).
    for c in 0..=cols {
        let x = x_start + c as f32 * col_width;
        page.push(v_line(
            x,
            y_start,
            y_start + rows as f32 * row_height,
            order,
        ));
        order += 1;
    }
    // Text in each cell.
    for r in 0..rows {
        for c in 0..cols {
            let x = x_start + c as f32 * col_width + 0.005;
            let y = y_start + r as f32 * row_height + 0.005;
            page.push(text_at(x, y, &format!("R{}C{}", r, c), order));
            order += 1;
        }
    }
    page
}

/// Build a borderless NxM stream page (text only, no lines).
fn build_stream_page(rows: usize, cols: usize) -> Vec<Primitive> {
    let mut page = Vec::new();
    let mut order = 0u64;

    let x_start = 0.05;
    let y_start = 0.05;
    let col_width = 0.85 / cols as f32;
    let row_height = 0.03;

    for r in 0..rows {
        for c in 0..cols {
            let x = x_start + c as f32 * col_width;
            let y = y_start + r as f32 * row_height;
            page.push(text_at(x, y, &format!("R{}C{}", r, c), order));
            order += 1;
        }
    }
    page
}

/// Build a lattice page with char_positions on every text primitive.
fn build_lattice_page_with_chars(rows: usize, cols: usize) -> Vec<Primitive> {
    let mut page = build_lattice_page(rows, cols);
    for prim in &mut page {
        if let PrimitiveKind::Text {
            ref content,
            ref mut char_positions,
            ..
        } = prim.kind
        {
            let n = content.len();
            if n > 0 {
                let char_width = prim.bbox.width / n as f32;
                let positions: Vec<crate::model::CharPosition> = content
                    .chars()
                    .enumerate()
                    .map(|(i, ch)| crate::model::CharPosition {
                        ch,
                        bbox: BoundingBox::new(
                            prim.bbox.x + i as f32 * char_width,
                            prim.bbox.y,
                            char_width,
                            prim.bbox.height,
                        ),
                    })
                    .collect();
                *char_positions = Some(positions);
            }
        }
    }
    page
}

fn run_bench(label: &str, iterations: u32, mut f: impl FnMut()) {
    // Warmup.
    for _ in 0..5 {
        f();
    }
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        f();
    }
    let elapsed = start.elapsed();
    let per_iter = elapsed / iterations;
    eprintln!(
        "  {:<45} {:>8.1}µs/iter  ({} iters in {:.1}ms)",
        label,
        per_iter.as_nanos() as f64 / 1000.0,
        iterations,
        elapsed.as_secs_f64() * 1000.0,
    );
}

#[test]
#[ignore]
fn bench_lattice_small() {
    eprintln!("\n=== Lattice 5×4 (20 cells) ===");
    let page = build_lattice_page(5, 4);
    let detector = TableDetector::default();
    let context = ctx();
    eprintln!("  Page primitives: {}", page.len());

    run_bench("full pipeline (detect)", 1000, || {
        let _ = detector.detect(&page, &context);
    });

    // Step-by-step breakdown.
    run_bench("step 1-2: extract + snap/merge", 2000, || {
        let raw = edges::extract_edges(&page, &detector.edge_config);
        let _ = edges::snap_and_merge(raw, &detector.edge_config);
    });

    let raw = edges::extract_edges(&page, &detector.edge_config);
    let merged = edges::snap_and_merge(raw, &detector.edge_config);

    run_bench("step 3: intersections", 2000, || {
        let _ = intersections::find_intersections(&merged, detector.intersection_tolerance);
    });

    let ints = intersections::find_intersections(&merged, detector.intersection_tolerance);

    run_bench("step 4: construct cells", 2000, || {
        let _ = cells::construct_cells(
            &ints,
            &merged,
            detector.intersection_tolerance,
            detector.edge_config.cell_validation,
        );
    });

    let raw_cells = cells::construct_cells(
        &ints,
        &merged,
        detector.intersection_tolerance,
        detector.edge_config.cell_validation,
    );

    run_bench("step 5: group into tables", 2000, || {
        let _ = cells::group_into_tables(
            &raw_cells,
            &page,
            detector.intersection_tolerance,
            detector.min_rows,
            detector.min_columns,
        );
    });
}

#[test]
#[ignore]
fn bench_lattice_large() {
    eprintln!("\n=== Lattice 20×10 (200 cells) ===");
    let page = build_lattice_page(20, 10);
    let detector = TableDetector::default();
    let context = ctx();
    eprintln!("  Page primitives: {}", page.len());

    run_bench("full pipeline (detect)", 500, || {
        let _ = detector.detect(&page, &context);
    });
}

#[test]
#[ignore]
fn bench_lattice_stress() {
    eprintln!("\n=== Lattice 50×20 (1000 cells) ===");
    let page = build_lattice_page(50, 20);
    let detector = TableDetector::default();
    let context = ctx();
    eprintln!("  Page primitives: {}", page.len());

    run_bench("full pipeline (detect)", 100, || {
        let _ = detector.detect(&page, &context);
    });
}

#[test]
#[ignore]
fn bench_stream_small() {
    eprintln!("\n=== Stream 5×4 (20 cells) ===");
    let page = build_stream_page(5, 4);
    let detector = TableDetector::default();
    let context = ctx();
    eprintln!("  Page primitives: {}", page.len());

    run_bench("full pipeline (detect)", 1000, || {
        let _ = detector.detect(&page, &context);
    });
}

#[test]
#[ignore]
fn bench_stream_large() {
    eprintln!("\n=== Stream 20×10 (200 cells) ===");
    let page = build_stream_page(20, 10);
    let detector = TableDetector::default();
    let context = ctx();
    eprintln!("  Page primitives: {}", page.len());

    run_bench("full pipeline (detect)", 500, || {
        let _ = detector.detect(&page, &context);
    });
}

#[test]
#[ignore]
fn bench_char_assignment_vs_primitive() {
    eprintln!("\n=== Char-level vs Primitive-level assignment (10×5 lattice) ===");
    let page_no_chars = build_lattice_page(10, 5);
    let page_with_chars = build_lattice_page_with_chars(10, 5);
    let detector = TableDetector::default();
    let context = ctx();

    run_bench("primitive-level (no char_positions)", 1000, || {
        let _ = detector.detect(&page_no_chars, &context);
    });
    run_bench("char-level (with char_positions)", 1000, || {
        let _ = detector.detect(&page_with_chars, &context);
    });
}
