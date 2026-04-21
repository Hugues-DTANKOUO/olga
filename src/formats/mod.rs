//! Format decoders — one module per supported document format.
//!
//! Shared XML utilities live in [`xml_utils`] and are reusable across decoders.

pub mod docx;
pub mod html;
pub mod pdf;
pub mod xlsx;
pub mod xml_utils;
