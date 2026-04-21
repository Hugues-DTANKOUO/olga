//! End-to-end pipeline benchmarks: decode + structure for each format.
//!
//! Measures the full pipeline (FormatDecoder::decode → StructureEngine::structure)
//! on real corpus fixtures to produce comparable per-page timings.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use olga::formats::docx::DocxDecoder;
use olga::formats::html::HtmlDecoder;
use olga::formats::pdf::PdfDecoder;
use olga::formats::xlsx::XlsxDecoder;
use olga::structure::layout::{HeuristicLayoutAnalyzer, LAParams};
use olga::structure::{StructureConfig, StructureEngine};
use olga::traits::FormatDecoder;

fn build_engine() -> StructureEngine {
    StructureEngine::new(StructureConfig::default())
        .with_default_detectors()
        .with_layout_analyzer(Box::new(HeuristicLayoutAnalyzer::new(LAParams::default())))
}

fn build_engine_no_layout() -> StructureEngine {
    StructureEngine::new(StructureConfig::default()).with_default_detectors()
}

// ---------------------------------------------------------------------------
// PDF benchmarks
// ---------------------------------------------------------------------------

fn bench_pdf_multi_page(c: &mut Criterion) {
    let data =
        std::fs::read("tests/corpus/pdf/multi_page_stress.pdf").expect("corpus file missing");

    c.bench_function("pipeline_pdf_multi_page", |b| {
        b.iter(|| {
            let decode_result = PdfDecoder.decode(black_box(data.clone())).unwrap();
            let _result = build_engine().structure(decode_result);
        });
    });
}

fn bench_pdf_structured_report(c: &mut Criterion) {
    let data =
        std::fs::read("tests/corpus/pdf/structured_report.pdf").expect("corpus file missing");

    c.bench_function("pipeline_pdf_structured_report", |b| {
        b.iter(|| {
            let decode_result = PdfDecoder.decode(black_box(data.clone())).unwrap();
            let _result = build_engine().structure(decode_result);
        });
    });
}

// ---------------------------------------------------------------------------
// DOCX benchmarks
// ---------------------------------------------------------------------------

fn bench_docx_mixed_content(c: &mut Criterion) {
    let data =
        std::fs::read("tests/corpus/docx/mixed_content_stress.docx").expect("corpus file missing");

    c.bench_function("pipeline_docx_mixed_content", |b| {
        b.iter(|| {
            let decode_result = DocxDecoder.decode(black_box(data.clone())).unwrap();
            let _result = build_engine_no_layout().structure(decode_result);
        });
    });
}

fn bench_docx_project_status(c: &mut Criterion) {
    let data = std::fs::read("tests/corpus/docx/project_status.docx").expect("corpus file missing");

    c.bench_function("pipeline_docx_project_status", |b| {
        b.iter(|| {
            let decode_result = DocxDecoder.decode(black_box(data.clone())).unwrap();
            let _result = build_engine_no_layout().structure(decode_result);
        });
    });
}

// ---------------------------------------------------------------------------
// HTML benchmarks
// ---------------------------------------------------------------------------

fn bench_html_complex_report(c: &mut Criterion) {
    let data = std::fs::read("tests/corpus/html/complex_report.html").expect("corpus file missing");

    c.bench_function("pipeline_html_complex_report", |b| {
        b.iter(|| {
            let decode_result = HtmlDecoder.decode(black_box(data.clone())).unwrap();
            let _result = build_engine_no_layout().structure(decode_result);
        });
    });
}

// ---------------------------------------------------------------------------
// XLSX benchmarks
// ---------------------------------------------------------------------------

fn bench_xlsx_multi_sheet(c: &mut Criterion) {
    let data =
        std::fs::read("tests/corpus/xlsx/multi_sheet_stress.xlsx").expect("corpus file missing");

    c.bench_function("pipeline_xlsx_multi_sheet", |b| {
        b.iter(|| {
            let decode_result = XlsxDecoder.decode(black_box(data.clone())).unwrap();
            let _result = build_engine_no_layout().structure(decode_result);
        });
    });
}

fn bench_xlsx_employee_directory(c: &mut Criterion) {
    let data =
        std::fs::read("tests/corpus/xlsx/employee_directory.xlsx").expect("corpus file missing");

    c.bench_function("pipeline_xlsx_employee_directory", |b| {
        b.iter(|| {
            let decode_result = XlsxDecoder.decode(black_box(data.clone())).unwrap();
            let _result = build_engine_no_layout().structure(decode_result);
        });
    });
}

criterion_group!(
    benches,
    bench_pdf_multi_page,
    bench_pdf_structured_report,
    bench_docx_mixed_content,
    bench_docx_project_status,
    bench_html_complex_report,
    bench_xlsx_multi_sheet,
    bench_xlsx_employee_directory,
);
criterion_main!(benches);
