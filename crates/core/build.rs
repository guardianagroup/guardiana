//! Tells cargo to rebuild when one of the interface texts changes.
//!
//! The three `i18n/*.json` files live in the panel crate but are embedded here with
//! `include_str!`, which does not always mark them as inputs: on 20 September 2026 a changed
//! column heading was still missing from the binary after a clean `cargo build`. Same fix as
//! `crates/panel/build.rs`, for the files this crate reads.
use std::fs;
use std::path::Path;

fn main() {
    // La identidad del programa: si cambia la clave pública y cargo no se entera, el binario sigue
    // llevando la anterior y `verify` comprueba firmas contra una clave que ya no firma nada. Pasó
    // a punto de pasar el 20 de septiembre de 2026, al hacer la clave nueva.
    println!("cargo:rerun-if-changed=../../build/pubkey/minisign.pub");
    let dir = Path::new("../panel/i18n");
    println!("cargo:rerun-if-changed={}", dir.display());
    let Ok(entradas) = fs::read_dir(dir) else {
        return;
    };
    for e in entradas.flatten() {
        println!("cargo:rerun-if-changed={}", e.path().display());
    }
}
