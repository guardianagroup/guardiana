//! `guardiana-lector`: list, extract and check a GUARDIANA vault with nothing but the published
//! format (`docs/BOVEDA.md`). Texts live in `es.json` next to this crate, never in the code.

use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use guardiana_vault_reader as lector;

fn texts() -> HashMap<String, String> {
    serde_json::from_str(include_str!("../es.json")).unwrap_or_default()
}

fn t<'a>(m: &'a HashMap<String, String>, k: &'a str) -> &'a str {
    m.get(k).map_or(k, String::as_str)
}

fn plain(m: &HashMap<String, String>, e: &lector::Error) -> String {
    match e {
        lector::Error::WrongKey => t(m, "err_clave").to_owned(),
        lector::Error::Tampered(q) => t(m, "err_alterado").replace("{que}", q),
        lector::Error::Format(q) => t(m, "err_formato").replace("{que}", q),
        lector::Error::NoObject(q) => t(m, "err_objeto").replace("{que}", q),
        lector::Error::BadWords(q) => t(m, "err_palabras").replace("{que}", q),
        lector::Error::Io(e) => e.to_string(),
    }
}

fn master(
    m: &HashMap<String, String>,
    h: &lector::Header,
    with_words: bool,
) -> Result<[u8; 32], lector::Error> {
    if with_words {
        print!("{}: ", t(m, "palabras"));
        io::stdout().flush()?;
        let mut s = String::new();
        io::stdin().lock().read_line(&mut s)?;
        let words: Vec<&str> = s.split_whitespace().collect();
        let e = lector::entropy_from_words(&words)?;
        lector::master_key(h, "palabras", &e)
    } else {
        let p = rpassword::prompt_password(format!("{}: ", t(m, "clave")))?;
        lector::master_key(h, "contrasena", p.as_bytes())
    }
}

fn run(args: &[String], m: &HashMap<String, String>) -> Result<(), lector::Error> {
    let with_words = args.iter().any(|a| a == "--palabras");
    let dest = args
        .iter()
        .position(|a| a == "--a")
        .and_then(|i| args.get(i + 1))
        .cloned();
    let pos: Vec<&String> = {
        let mut skip = false;
        args.iter()
            .filter(|a| {
                if skip {
                    skip = false;
                    return false;
                }
                if *a == "--a" {
                    skip = true;
                    return false;
                }
                !a.starts_with("--")
            })
            .collect()
    };
    match (pos.first().map(|s| s.as_str()), pos.get(1), pos.get(2)) {
        (Some("lista"), Some(d), _) => {
            let h = lector::header(Path::new(d))?;
            let k = master(m, &h, with_words)?;
            let items = lector::list(Path::new(d), &k)?;
            if items.is_empty() {
                println!("{}", t(m, "vacia"));
            } else {
                println!("{}", t(m, "cab"));
                for i in items {
                    println!(
                        "{} · {:>10} · {} · {}",
                        i.id,
                        i.tamano,
                        &i.guardado[..16],
                        i.nombre
                    );
                }
            }
            Ok(())
        }
        (Some("sacar"), Some(d), Some(q)) => {
            let h = lector::header(Path::new(d))?;
            let k = master(m, &h, with_words)?;
            let items = lector::list(Path::new(d), &k)?;
            let item = items
                .iter()
                .find(|i| &i.id == *q || &i.nombre == *q)
                .ok_or_else(|| lector::Error::NoObject((*q).clone()))?;
            let out = dest.map_or_else(|| PathBuf::from(&item.nombre), PathBuf::from);
            if out.exists() {
                println!(
                    "{}",
                    t(m, "existe").replace("{ruta}", &out.display().to_string())
                );
                return Ok(());
            }
            let mut f = std::fs::File::create(&out)?;
            let it = lector::extract(Path::new(d), &k, q, &mut f)?;
            println!(
                "{}",
                t(m, "sacado")
                    .replace("{ruta}", &out.display().to_string())
                    .replace("{tamano}", &it.tamano.to_string())
                    .replace("{sha}", &it.sha256)
            );
            Ok(())
        }
        (Some("registro"), Some(d), _) => {
            let h = lector::header(Path::new(d))?;
            let (lines, check) = lector::log(Path::new(d), &h.id)?;
            println!("{}", t(m, "registro_cab"));
            for l in &lines {
                println!(
                    "{} · {} · {} · {} · {}",
                    l.n,
                    l.hora,
                    l.accion,
                    if l.objeto.is_empty() {
                        "—"
                    } else {
                        &l.objeto
                    },
                    l.detalle
                );
            }
            match check {
                Ok(n) => println!("{}", t(m, "registro_ok").replace("{n}", &n.to_string())),
                Err(n) => println!("{}", t(m, "registro_rota").replace("{n}", &n.to_string())),
            }
            Ok(())
        }
        _ => {
            println!("{}", t(m, "uso"));
            Ok(())
        }
    }
}

fn main() {
    let m = texts();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = run(&args, &m) {
        eprintln!("{}", plain(&m, &e));
        std::process::exit(1);
    }
}
