//! Public API — the user-facing surface of Olga.
//!
//! This module wraps the internal pipeline (decoders, structure engine,
//! renderers) behind a stable, ergonomic API that library consumers can
//! use without reaching into the crate's internals.
//!
//! # Quick start
//!
//! ```no_run
//! use olga::api::Document;
//!
//! let doc = Document::open("report.pdf")?;
//!
//! // Pages are 1-based on the public API.
//! for page in doc.pages() {
//!     println!("page {} — {} chars", page.number(), page.text().len());
//! }
//!
//! // Or grab per-page text as a map.
//! let by_page = doc.text_by_page();
//! for (n, content) in &by_page {
//!     println!("page {n}: {} chars", content.len());
//! }
//! # Ok::<(), olga::api::Error>(())
//! ```
//!
//! # Design
//!
//! - `Document` owns the decoded input and lazily computes views on demand.
//!   It is cheap to clone conceptually (heavy work is cached), but the public
//!   type is *not* `Clone` — share it behind an `Arc` if needed.
//! - `Page` is a light handle: it borrows the shared document state via an
//!   internal `Arc`, so it can be freely cloned and passed across threads.
//! - All string indices on page numbers are **1-based** on the public API.
//! - Coordinates on bounding boxes remain in the internal 0.0–1.0 normalized
//!   space (see [`crate::model::BoundingBox`]).
//! - The API is `Send + Sync`; internal caches use `OnceLock`.

mod chunks;
mod document;
mod format;
mod links;
mod outline;
mod page;
mod processability;
mod search;
mod tables;

pub use chunks::Chunk;
pub use document::Document;
pub use format::{Format, detect_format};
pub use links::Link;
pub use outline::OutlineEntry;
pub use page::Page;
pub use processability::{Health, HealthIssue, Processability};
pub use search::SearchHit;
pub use tables::{Table, TableCell};

// Flat re-exports so consumers don't have to import from deep paths.
pub use crate::error::{IdpError as Error, IdpResult as Result, Warning, WarningKind};
pub use crate::model::{
    BoundingBox, DocumentMetadata, ExtractedImage, ImageFormat, PageDimensions,
};

#[cfg(test)]
mod tests;
