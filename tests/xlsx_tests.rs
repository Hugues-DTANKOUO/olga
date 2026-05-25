// Gated on the `xlsx` cargo feature per v0.1.2 [features] block.
// When `xlsx` is disabled, this test binary compiles to an empty
// shell with zero tests — no failure, no false-positive coverage.
#![cfg(feature = "xlsx")]

mod support;

#[path = "xlsx/foundation.rs"]
mod foundation;
#[path = "xlsx/raw_path.rs"]
mod raw_path;
