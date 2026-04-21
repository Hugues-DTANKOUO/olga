#![allow(dead_code)]
#![allow(unused_imports)]

#[path = "xlsx/core.rs"]
mod core;
#[path = "xlsx/layout.rs"]
mod layout;
#[path = "xlsx/links_formulas.rs"]
mod links_formulas;
#[path = "xlsx/manifest.rs"]
mod manifest;
#[path = "xlsx/sheet_parts.rs"]
mod sheet_parts;
#[path = "xlsx/watchlist.rs"]
mod watchlist;

pub use core::{build_xlsx, build_xlsx_with_invalid_shared_strings, col_to_letter, decode_xlsx};
pub use layout::{
    build_xlsx_with_hidden_first_row_heuristic_header, build_xlsx_with_hidden_rows_and_cols,
};
pub use links_formulas::{
    build_xlsx_with_formula_without_cached_value, build_xlsx_with_sheet_hyperlink,
};
pub use manifest::build_manifest_fraction_canary_xlsx;
pub use sheet_parts::{
    build_xlsx_with_comments_on_empty_cells, build_xlsx_with_missing_sheet_part,
    build_xlsx_with_native_table,
};
pub use watchlist::build_watchlist_canary_xlsx;
