//! Embeds the browser's own pages (`interfaz/`) into the program, and on Windows the icon, the
//! manifest and the version information.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::{env, fs};

fn archivos(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(lista) = fs::read_dir(dir) else {
        return;
    };
    let mut lista: Vec<PathBuf> = lista.filter_map(|e| e.ok().map(|e| e.path())).collect();
    lista.sort();
    for p in lista {
        if p.is_dir() {
            archivos(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn main() {
    let manifiesto = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let raiz = manifiesto.join("interfaz");
    let mut lista = Vec::new();
    archivos(&raiz, &mut lista);
    let mut codigo = String::from("/// The browser's own pages, as they are in `interfaz/`.\npub const ARCHIVOS: &[(&str, &[u8])] = &[\n");
    for p in &lista {
        let rel = p
            .strip_prefix(&raiz)
            .unwrap_or(p)
            .to_string_lossy()
            .replace('\\', "/");
        let _ = writeln!(
            codigo,
            "    ({rel:?}, include_bytes!({:?})),",
            p.to_string_lossy()
        );
    }
    codigo.push_str("];\n");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap_or_default());
    let _ = fs::write(out.join("interfaz.rs"), codigo);
    println!("cargo:rerun-if-changed=interfaz");
    println!("cargo:rerun-if-changed=recursos");

    #[cfg(windows)]
    {
        let mut r = winresource::WindowsResource::new();
        r.set_icon("recursos/guardiana-zero.ico")
            .set_manifest_file("recursos/guardiana-zero.manifest")
            .set("ProductName", "GUARDIANA ZERO")
            .set("FileDescription", "GUARDIANA ZERO")
            .set("CompanyName", "Guardiana")
            .set("LegalCopyright", "GPL-3.0-or-later")
            .set("OriginalFilename", "guardiana-zero.exe");
        if let Err(e) = r.compile() {
            println!("cargo:warning=no se pudo incrustar el icono: {e}");
        }
    }
}
