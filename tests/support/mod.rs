// Per v0.1.2 [features] block : per-format support modules are
// gated on their respective features so the parent test crate
// (e.g. `e2e_structure_stress`) compiles cleanly across feature
// combinations. `contracts` is feature-agnostic (it only consumes
// `olga::error::Warning` + `olga::traits::FormatDecoder`) and stays
// unconditional. The other 3 modules reach into a format-specific
// decoder (`HtmlDecoder` / `XlsxDecoder`) or construct format-
// specific fixtures and therefore must be gated.
pub mod contracts;
#[cfg(feature = "docx")]
pub mod docx;
#[cfg(feature = "html")]
pub mod html;
#[cfg(feature = "xlsx")]
pub mod xlsx;
