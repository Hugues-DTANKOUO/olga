//! Format decoders — one module per supported document format.
//!
//! Each decoder module is gated behind its corresponding cargo feature
//! (per v0.1.2 `[features]` block) — `docx` / `html` / `pdf` / `xlsx`.
//! The default feature set enables all four for backward compatibility
//! with v0.1.1 ; consumers can opt out via `default-features = false`.
//!
//! Shared XML utilities live in `xml_utils` and are reusable across
//! the OOXML decoders (DOCX + XLSX) ; gated on the umbrella `_ooxml`
//! private feature that both `docx` and `xlsx` enable transitively.

#[cfg(feature = "docx")]
pub mod docx;
#[cfg(feature = "html")]
pub mod html;
#[cfg(feature = "pdf")]
pub mod pdf;
#[cfg(feature = "xlsx")]
pub mod xlsx;
#[cfg(any(feature = "docx", feature = "xlsx"))]
pub mod xml_utils;
