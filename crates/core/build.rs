//! Tells cargo to rebuild when one of the interface texts changes.
//!
//! The three `i18n/*.json` files live in the panel crate but are embedded here with
//! `include_str!`, which does not always mark them as inputs: on 20 September 2026 a changed
//! column heading was still missing from the binary after a clean `cargo build`. Same fix as
//! `crates/panel/build.rs`, for the files this crate reads.
use std::fs;
use std::path::Path;

fn main() {
    let dir = Path::new("../panel/i18n");
    println!("cargo:rerun-if-changed={}", dir.display());
    let Ok(entradas) = fs::read_dir(dir) else {
        return;
    };
    for e in entradas.flatten() {
        println!("cargo:rerun-if-changed={}", e.path().display());
    }
}
