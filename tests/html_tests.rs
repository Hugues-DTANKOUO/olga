// Gated on the `html` cargo feature per v0.1.2 [features] block.
// When `html` is disabled, this test binary compiles to an empty
// shell with zero tests — no failure, no false-positive coverage.
#![cfg(feature = "html")]

mod support;

#[path = "html/fallbacks.rs"]
mod fallbacks;
#[path = "html/foundation.rs"]
mod foundation;
#[path = "html/robustness.rs"]
mod robustness;
#[path = "html/structures.rs"]
mod structures;
