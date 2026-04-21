//! XLSX format decoder.
//!
//! Parses Office Open XML spreadsheets by:
//! 1. Inspecting workbook/sheet structure directly from raw OOXML
//! 2. Streaming simple worksheet values from raw sheet XML when safe
//! 3. Falling back to the workbook decoder for richer cell features that still
//!    require style-aware value decoding
//! 4. Emitting Primitives with TableCell and native hyperlink hints
//! 5. Detecting header rows via a fallback heuristic on the first visible
//!    all-string row when native SpreadsheetML table metadata is absent
//! 6. Handling merged cells from the raw XML
//!
//! Design: each sheet maps to a separate page (page=0 for first sheet, etc.).
//! Cell positions are normalized to 0.0–1.0 based on total rows/cols in the sheet.
//! These coordinates are reconstructed logical-grid positions optimized for
//! table semantics, not for reproducing spreadsheet rendering. When the
//! decoder falls back to header inference instead of native table metadata,
//! it emits an explicit warning so callers can distinguish heuristic
//! structure from observed SpreadsheetML semantics. When a sheet uses richer
//! cell features that the raw path does not yet decode losslessly, the
//! decoder keeps fidelity by falling back to the workbook decoder for that
//! sheet instead of inventing values. If a streamed formula cell exposes no
//! cached value, the decoder now warns explicitly and skips the cell content
//! rather than fabricating a result.

mod number_format;
mod sheets;
mod types;
mod workbook_support;

use crate::error::{IdpError, IdpResult};
use crate::model::*;
use crate::traits::{DecodeResult, FormatDecoder};

use sheets::{inspect_workbook_sheet_count, process_sheets};

const XLSX_EXTENSIONS: &[&str] = &["xlsx", "xlsm"];

// ---------------------------------------------------------------------------
// XlsxDecoder — the public entry point
// ---------------------------------------------------------------------------

/// XLSX format decoder.
pub struct XlsxDecoder;

impl XlsxDecoder {
    pub fn new() -> Self {
        Self
    }
}

impl Default for XlsxDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatDecoder for XlsxDecoder {
    fn name(&self) -> &str {
        "XLSX"
    }

    fn supported_extensions(&self) -> &[&str] {
        XLSX_EXTENSIONS
    }

    fn metadata(&self, data: &[u8]) -> IdpResult<DocumentMetadata> {
        let sheet_count = inspect_workbook_sheet_count(data).map_err(|e| match e {
            IdpError::MissingPart(_) | IdpError::Decode { .. } => {
                IdpError::UnsupportedFormat(format!("Not a valid XLSX: {e}"))
            }
            other => other,
        })?;

        Ok(DocumentMetadata {
            format: DocumentFormat::Xlsx,
            page_count: sheet_count.max(1),
            page_count_provenance: PaginationProvenance::observed(PaginationBasis::WorkbookSheets),
            page_dimensions: vec![],
            encrypted: false,
            text_extraction_allowed: true,
            file_size: data.len() as u64,
            title: None,
        })
    }

    fn decode(&self, data: Vec<u8>) -> IdpResult<DecodeResult> {
        let mut primitives = Vec::new();
        let mut warnings = Vec::new();

        let (sheet_count, title) = process_sheets(&data, &mut primitives, &mut warnings)?;

        let metadata = DocumentMetadata {
            format: DocumentFormat::Xlsx,
            page_count: sheet_count.max(1),
            page_count_provenance: PaginationProvenance::observed(PaginationBasis::WorkbookSheets),
            page_dimensions: vec![],
            encrypted: false,
            text_extraction_allowed: true,
            file_size: data.len() as u64,
            title,
        };

        // XLSX has no image extraction in v1; sheet-level media ships later.
        Ok(DecodeResult {
            metadata,
            primitives,
            warnings,
            images: Vec::new(),
        })
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
