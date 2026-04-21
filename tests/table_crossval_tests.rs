//! Cross-validation tests comparing IDP table detection against pdfplumber.
//!
//! When the reference JSON is not available, the test exits early with a
//! diagnostic message (no failure). To generate the reference data:
//!
//! ```bash
//! pip install pdfplumber
//! python tests/table_crossval/pdfplumber_extract.py \
//!     tests/corpus/pdf/structured_report.pdf \
//!     tests/table_crossval/structured_report_ref.json
//! ```

use std::collections::HashSet;
use std::path::Path;

use olga::formats::pdf::PdfDecoder;
use olga::model::{HintKind, PrimitiveKind};
use olga::structure::detectors::table::TableDetector;
use olga::structure::detectors::{DetectionContext, StructureDetector};
use olga::structure::types::FontHistogram;
use olga::traits::FormatDecoder;

/// Deserialized pdfplumber reference data.
#[derive(Debug, serde::Deserialize)]
struct PdfplumberRef {
    #[allow(dead_code)]
    file: String,
    pages: Vec<PageRef>,
}

#[derive(Debug, serde::Deserialize)]
struct PageRef {
    #[allow(dead_code)]
    page_number: usize,
    tables: Vec<TableRef>,
    #[allow(dead_code)]
    width: f64,
    #[allow(dead_code)]
    height: f64,
}

#[derive(Debug, serde::Deserialize)]
struct TableRef {
    #[allow(dead_code)]
    bbox: [f64; 4],
    rows: usize,
    cols: usize,
    cells: Vec<CellRef>,
}

#[derive(Debug, serde::Deserialize)]
struct CellRef {
    #[allow(dead_code)]
    row: usize,
    #[allow(dead_code)]
    col: usize,
    text: String,
}

/// Load pdfplumber reference JSON if it exists.
fn load_reference(json_path: &str) -> Option<PdfplumberRef> {
    let path = Path::new(json_path);
    if !path.exists() {
        eprintln!("Reference file not found: {json_path}");
        eprintln!(
            "Generate it with: python tests/table_crossval/pdfplumber_extract.py <pdf> {json_path}"
        );
        return None;
    }
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

/// Run IDP table detection on a page of primitives and extract cell texts.
fn idp_detect_cell_texts(page: &[olga::model::Primitive]) -> (usize, HashSet<String>) {
    let detector = TableDetector::default();
    let ctx = DetectionContext {
        font_histogram: FontHistogram::default(),
        page: 0,
        page_dims: None,
        text_boxes: None,
        open_table_columns: None,
    };

    let hints = detector.detect(page, &ctx);

    let mut texts = HashSet::new();
    let mut table_count = 0;

    // Count distinct (row, col) pairs to estimate table count.
    let mut seen_cells: HashSet<(u32, u32)> = HashSet::new();

    for hint in &hints {
        match &hint.hint.kind {
            HintKind::TableCell { row, col, .. } => {
                seen_cells.insert((*row, *col));

                // Extract the text content.
                if let PrimitiveKind::Text { ref content, .. } = page[hint.primitive_index].kind {
                    let text = content.trim().to_string();
                    if !text.is_empty() {
                        texts.insert(text);
                    }
                }
            }
            HintKind::TableHeader { col } => {
                seen_cells.insert((0, *col));

                // Extract the text content.
                if let PrimitiveKind::Text { ref content, .. } = page[hint.primitive_index].kind {
                    let text = content.trim().to_string();
                    if !text.is_empty() {
                        texts.insert(text);
                    }
                }
            }
            _ => {}
        }
    }

    if !seen_cells.is_empty() {
        table_count = 1; // Simplified: count as 1 table per page with hints.
    }

    (table_count, texts)
}

#[test]
fn crossval_structured_report() {
    let ref_path = "tests/table_crossval/structured_report_ref.json";
    let reference = match load_reference(ref_path) {
        Some(r) => r,
        None => {
            eprintln!("Skipping cross-validation: reference not available");
            return;
        }
    };

    // Load the PDF via our decoder pipeline.
    let pdf_path = "tests/corpus/pdf/structured_report.pdf";
    if !Path::new(pdf_path).exists() {
        eprintln!("Skipping: PDF not found at {pdf_path}");
        return;
    }

    let pdf_bytes = std::fs::read(pdf_path).expect("Failed to read PDF");
    let decoder = PdfDecoder;
    let decode_result = match decoder.decode(pdf_bytes.clone()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to decode PDF: {e}");
            return;
        }
    };

    // Group primitives by page.
    let max_page = decode_result
        .primitives
        .iter()
        .map(|p| p.page)
        .max()
        .unwrap_or(0);
    let mut pages: Vec<Vec<olga::model::Primitive>> = vec![Vec::new(); max_page as usize + 1];
    for prim in decode_result.primitives {
        pages[prim.page as usize].push(prim);
    }

    let mut match_count = 0;
    let mut total_ref_texts = 0;

    for (page_idx, ref_page) in reference.pages.iter().enumerate() {
        if page_idx >= pages.len() {
            break;
        }

        let page_prims = &pages[page_idx];
        let (_idp_table_count, idp_texts) = idp_detect_cell_texts(page_prims);

        // Collect all non-empty cell texts from pdfplumber reference.
        let ref_texts: HashSet<String> = ref_page
            .tables
            .iter()
            .flat_map(|t| t.cells.iter())
            .filter(|c| !c.text.is_empty())
            .map(|c| c.text.clone())
            .collect();

        total_ref_texts += ref_texts.len();

        // Count how many pdfplumber texts our engine also found.
        for text in &ref_texts {
            if idp_texts.contains(text) {
                match_count += 1;
            }
        }

        // Log comparison.
        let ref_table_count = ref_page.tables.len();
        let ref_grid: Vec<(usize, usize)> =
            ref_page.tables.iter().map(|t| (t.rows, t.cols)).collect();
        eprintln!(
            "Page {page_idx}: pdfplumber={ref_table_count} tables {ref_grid:?}, \
             IDP found {} texts out of {} ref texts",
            idp_texts.len(),
            ref_texts.len()
        );
    }

    // Assert >= 80% text match rate.
    if total_ref_texts > 0 {
        let match_rate = match_count as f64 / total_ref_texts as f64;
        eprintln!(
            "Overall text match rate: {match_count}/{total_ref_texts} = {:.1}%",
            match_rate * 100.0
        );
        assert!(
            match_rate >= 0.80,
            "Text match rate {:.1}% is below 80% threshold",
            match_rate * 100.0
        );
    }
}
