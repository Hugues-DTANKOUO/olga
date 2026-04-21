//! Core data model for the Olga pipeline.
//!
//! Split by responsibility:
//! - [`geometry`]: Spatial types (Point, BoundingBox, PageDimensions)
//! - [`primitives`]: The intermediate model (Primitive, PrimitiveKind, Color, etc.)
//! - [`hints`]: Semantic annotations (SemanticHint, HintKind, HintSource)
//! - [`metadata`]: Document-level metadata (DocumentMetadata, DocumentFormat)
//! - [`document`]: The output model (DocumentNode tree)

pub mod document;
pub mod geometry;
pub mod hints;
pub mod image;
pub mod metadata;
pub mod primitives;
pub mod provenance;

// Re-export the most commonly used types at the model level.
// External consumers just write `use olga::model::*`.
pub use document::{DocumentNode, InlineSpan, NodeKind};
pub use geometry::{BoundingBox, PageDimensions, Point, RawBox};
pub use hints::{HintKind, HintSource, SemanticHint};
pub use image::ExtractedImage;
pub use metadata::{DocumentFormat, DocumentMetadata};
pub use primitives::{CharPosition, Color, ImageFormat, Primitive, PrimitiveKind, TextDirection};
pub use provenance::{
    GeometryProvenance, GeometrySpace, PaginationBasis, PaginationProvenance, StructureSource,
    ValueOrigin,
};
