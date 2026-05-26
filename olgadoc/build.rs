//! Cargo build script for `olgadoc` (Python bindings via `pyo3`).
//!
//! ## Why this build script exists
//!
//! On macOS, building a `cdylib` that references Python symbols
//! (`_PyBaseObject_Type`, `_PyDict_New`, etc.) requires the linker
//! flag `-undefined dynamic_lookup` so that the unresolved symbols
//! are deferred to the Python interpreter at module-import time.
//! This is the canonical pattern for Python extension modules on
//! Darwin — the interpreter provides `libpython.dylib` symbols at
//! `dlopen` time, not at link time.
//!
//! `pyo3`'s `extension-module` feature already emits `-nodefaultlibs`
//! on macOS, but as of `pyo3 0.28` it does NOT emit the companion
//! `-undefined dynamic_lookup` when invoked outside the `maturin`
//! driver (i.e. when the user runs `cargo build` / `cargo test`
//! directly from the workspace root). Without it, `cc` fails with
//! ~250 "Undefined symbols for architecture arm64" errors at link
//! time for the `cdylib` target.
//!
//! This build script makes the link arg unconditional on macOS so
//! that `cargo build` produces a loadable `.dylib` even when
//! `maturin` isn't the driver. The flag is harmless under `maturin`
//! (which sets up the same link discipline via its own build hooks)
//! and is a no-op on Linux / Windows where Python extension modules
//! don't use the `dynamic_lookup` pattern.

fn main() {
    // Re-run only when this script itself changes — pyo3 link
    // discipline is OS-keyed, not source-file-keyed.
    println!("cargo:rerun-if-changed=build.rs");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "macos" {
        // The two flags together tell `ld` : "don't fail on
        // undefined Python symbols ; they'll be resolved at runtime
        // when the interpreter dlopens this .dylib".
        println!("cargo:rustc-cdylib-link-arg=-undefined");
        println!("cargo:rustc-cdylib-link-arg=dynamic_lookup");
    }
}
