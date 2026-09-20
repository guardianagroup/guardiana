//! Tells cargo to rebuild when a file under `static/` changes.
//!
//! Without this, editing the panel's CSS or JavaScript and running `cargo build` can finish with
//! "Finished" and no work done: `include_str!` did not always mark those files as inputs, so the
//! binary kept serving the previous version. It cost a confusing half hour on 20 September 2026 --
//! the panel was serving a temporary change that had already been reverted in the file.
use std::fs;
use std::path::Path;

fn main() {
    marcar(Path::new("static"));
}

fn marcar(dir: &Path) {
    println!("cargo:rerun-if-changed={}", dir.display());
    let Ok(entradas) = fs::read_dir(dir) else {
        return;
    };
    for e in entradas.flatten() {
        let p = e.path();
        if p.is_dir() {
            marcar(&p);
        } else {
            println!("cargo:rerun-if-changed={}", p.display());
        }
    }
}
