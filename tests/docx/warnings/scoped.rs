use olga::error::WarningKind;
use olga::formats::docx::DocxDecoder;
use olga::model::*;
use olga::traits::FormatDecoder;

use crate::support::docx::{build_docx, build_docx_with_parts_and_rels};

#[path = "scoped/links_styles.rs"]
mod links_styles;
#[path = "scoped/malformed.rs"]
mod malformed;
#[path = "scoped/media_tracking.rs"]
mod media_tracking;
#[path = "scoped/numbering.rs"]
mod numbering;
#[path = "scoped/table_layout.rs"]
mod table_layout;
