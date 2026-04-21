use olga::error::WarningKind;
use olga::formats::docx::DocxDecoder;
use olga::model::*;
use olga::traits::FormatDecoder;

use crate::support::docx::{build_docx, build_docx_with_parts_and_rels};

#[path = "media/paragraph_annotations.rs"]
mod paragraph_annotations;
#[path = "media/paragraph_hints.rs"]
mod paragraph_hints;
#[path = "media/stories.rs"]
mod stories;
