use olga::error::WarningKind;
use olga::formats::docx::DocxDecoder;
use olga::model::*;
use olga::traits::FormatDecoder;

use crate::support::docx::{build_docx, build_docx_with_parts, build_docx_with_parts_and_rels};

#[path = "tables/basic.rs"]
mod basic;
#[path = "tables/content.rs"]
mod content;
#[path = "tables/merges.rs"]
mod merges;
#[path = "tables/nested.rs"]
mod nested;
