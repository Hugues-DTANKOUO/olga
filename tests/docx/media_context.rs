use olga::error::WarningKind;
use olga::formats::docx::DocxDecoder;
use olga::model::*;
use olga::traits::FormatDecoder;

use crate::support::docx::build_docx_with_parts_and_rels;

#[path = "media_context/ambiguity.rs"]
mod ambiguity;
#[path = "media_context/table_context.rs"]
mod table_context;
