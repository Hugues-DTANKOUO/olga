use criterion::{Criterion, black_box, criterion_group, criterion_main};
use olga::model::{BoundingBox, Primitive, PrimitiveKind};
use olga::structure::layout::{HeuristicLayoutAnalyzer, LAParams, LayoutAnalyzer};

/// Generate n text primitives spread across a page in a single-column layout.
///
/// Creates n text items with varying Y positions (reading order) and fixed X position,
/// simulating a vertical column of text items commonly found in documents.
fn generate_lines(n: usize) -> Vec<Primitive> {
    let mut lines = Vec::with_capacity(n);
    let line_height = 0.025; // 2.5% of page height between lines
    let mut y = 0.05;

    for i in 0..n {
        let text = format!("Line {} of document content", i + 1);
        lines.push(Primitive::observed(
            PrimitiveKind::Text {
                content: text,
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
            BoundingBox::new(0.05, y, 0.80, 0.02),
            0, // single page
            i as u64,
        ));
        y += line_height;
    }

    lines
}

fn bench_layout_100_lines(c: &mut Criterion) {
    c.bench_function("layout_100_lines", |b| {
        b.iter(|| {
            let lines = black_box(generate_lines(100));
            let analyzer = HeuristicLayoutAnalyzer::new(LAParams::default());
            let _ = analyzer.analyze(&lines);
        });
    });
}

fn bench_layout_500_lines(c: &mut Criterion) {
    c.bench_function("layout_500_lines", |b| {
        b.iter(|| {
            let lines = black_box(generate_lines(500));
            let analyzer = HeuristicLayoutAnalyzer::new(LAParams::default());
            let _ = analyzer.analyze(&lines);
        });
    });
}

fn bench_layout_1000_lines(c: &mut Criterion) {
    c.bench_function("layout_1000_lines", |b| {
        b.iter(|| {
            let lines = black_box(generate_lines(1000));
            let analyzer = HeuristicLayoutAnalyzer::new(LAParams::default());
            let _ = analyzer.analyze(&lines);
        });
    });
}

criterion_group!(
    benches,
    bench_layout_100_lines,
    bench_layout_500_lines,
    bench_layout_1000_lines
);
criterion_main!(benches);
