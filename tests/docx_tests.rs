//! Integration tests for the DOCX decoder.
//!
//! These tests create minimal DOCX files in-memory (ZIP archives with XML)
//! and verify the decoder produces correct Primitives and SemanticHints.

mod support;

#[path = "docx/foundation.rs"]
mod foundation;
#[path = "docx/media.rs"]
mod media;
#[path = "docx/media_context.rs"]
mod media_context;
#[path = "docx/metadata.rs"]
mod metadata;
#[path = "docx/secondary_parts.rs"]
mod secondary_parts;
#[path = "docx/semantics.rs"]
mod semantics;
#[path = "docx/tables.rs"]
mod tables;
#[path = "docx/warnings.rs"]
mod warnings;
