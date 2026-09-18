//! `guardiana boveda …`: the encrypted container on this machine (DECISIONES #103 and #126).
//! Every action goes to the vault's own chained log. Nothing here looks at the licence: cancelling
//! never blocks decryption, by construction.

use std::error::Error;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use guardiana_core::i18n;
use guardiana_vault::{self as vault, KdfParams, Vault};
use zeroize::Zeroizing;

use crate::args::Opts;

/// Who asked, as written in the log.
const QUIEN: &str = "terminal";
/// Shortest master password accepted.
const MIN_PASSWORD: usize = 10;

/// `~/.guardiana/boveda` on Unix, `%USERPROFILE%\Guardiana\boveda` on Windows: the user's, not
/// the service's data folder, because the vault belongs to a person.
pub(crate) fn default_dir() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE")
            .map_or_else(|| PathBuf::from("."), PathBuf::from)
            .join("Guardiana")
            .join("boveda")
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME")
            .map_or_else(|| PathBuf::from("."), PathBuf::from)
            .join(".guardiana")
            .join("boveda")
    }
}

fn dir(opts: &Opts) -> PathBuf {
    opts.get("ruta").map_or_else(default_dir, PathBuf::from)
}

fn password(prompt: &str) -> Result<Zeroizing<Vec<u8>>, Box<dyn Error>> {
    // The prompt goes through stdout (UTF-8), not through rpassword: on Windows the console
    // would print the accents in the OEM code page ("Contrase├▒a").
    print!("{prompt}: ");
    io::stdout().flush()?;
    let s = Zeroizing::new(rpassword::read_password()?);
    println!();
    Ok(Zeroizing::new(s.as_bytes().to_vec()))
}

/// Ask twice; `None` when the two do not match or it is too short (already explained).
fn new_password(t: &i18n::Texts) -> Result<Option<Zeroizing<Vec<u8>>>, Box<dyn Error>> {
    let a = password(t.cli("boveda_clave_nueva"))?;
    if a.len() < MIN_PASSWORD {
        println!("{}", t.cli("boveda_clave_corta"));
        return Ok(None);
    }
    let b = password(t.cli("boveda_clave_repite"))?;
    if *a != *b {
        println!("{}", t.cli("boveda_clave_distinta"));
        return Ok(None);
    }
    Ok(Some(a))
}

fn line(prompt: &str) -> Result<String, Box<dyn Error>> {
    print!("{prompt}: ");
    io::stdout().flush()?;
    let mut s = String::new();
    io::stdin().lock().read_line(&mut s)?;
    Ok(s.trim().to_owned())
}

fn size(n: u64) -> String {
    if n >= 1 << 20 {
        format!("{:.1} MB", n as f64 / f64::from(1u32 << 20))
    } else if n >= 1 << 10 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{n} B")
    }
}

/// Vault errors in the user's words.
fn plain(t: &i18n::Texts, e: &vault::Error) -> String {
    match e {
        vault::Error::WrongKey => t.cli("boveda_err_clave").to_owned(),
        vault::Error::Tampered(q) => t.cli("boveda_err_alterado").replace("{que}", q),
        vault::Error::BadWords(q) => t.cli("boveda_err_palabras").replace("{que}", q),
        vault::Error::BadShare(q) => t.cli("boveda_err_trozo").replace("{que}", q),
        vault::Error::Exists(p) => t
            .cli("boveda_err_existe")
            .replace("{ruta}", &p.display().to_string()),
        vault::Error::NotFound(p) => t
            .cli("boveda_err_no_hay")
            .replace("{ruta}", &p.display().to_string()),
        vault::Error::NoObject(q) => t.cli("boveda_err_objeto").replace("{que}", q),
        vault::Error::ChainBroken(n) => t.cli("boveda_err_cadena").replace("{n}", &n.to_string()),
        other => other.to_string(),
    }
}

fn open(t: &i18n::Texts, d: &Path) -> Result<Vault, Box<dyn Error>> {
    Vault::peek(d).map_err(|e| plain(t, &e))?;
    let p = password(t.cli("boveda_clave"))?;
    Vault::open(d, &p, QUIEN).map_err(|e| plain(t, &e).into())
}

fn print_words(t: &i18n::Texts, w: &[&str]) {
    println!("\n{}\n", t.cli("boveda_palabras_titulo"));
    for (i, chunk) in w.chunks(6).enumerate() {
        let row: Vec<String> = chunk
            .iter()
            .enumerate()
            .map(|(j, x)| format!("{:>2}. {x:<10}", i * 6 + j + 1))
            .collect();
        println!("  {}", row.join(" "));
    }
    println!();
}

fn crear(t: &i18n::Texts, d: &Path) -> Result<(), Box<dyn Error>> {
    if d.join("boveda.json").exists() {
        return Err(plain(t, &vault::Error::Exists(d.to_owned())).into());
    }
    let Some(p) = new_password(t)? else {
        return Ok(());
    };
    let created = Vault::create(d, &p, KdfParams::default(), QUIEN).map_err(|e| plain(t, &e))?;
    println!(
        "{}",
        t.cli("boveda_creada")
            .replace("{ruta}", &d.display().to_string())
    );
    print_words(t, &created.words);
    let _ = line(t.cli("boveda_palabras_confirma"))?;
    Ok(())
}

fn guardar(t: &i18n::Texts, d: &Path, file: &Path) -> Result<(), Box<dyn Error>> {
    let v = open(t, d)?;
    let item = v.add_file(file, QUIEN).map_err(|e| plain(t, &e))?;
    println!(
        "{}",
        t.cli("boveda_guardado")
            .replace("{id}", &item.id)
            .replace("{nombre}", &item.meta.nombre)
            .replace("{tamano}", &size(item.meta.tamano))
    );
    Ok(())
}

fn lista(t: &i18n::Texts, d: &Path) -> Result<(), Box<dyn Error>> {
    let v = open(t, d)?;
    let items = v.list().map_err(|e| plain(t, &e))?;
    if items.is_empty() {
        println!("{}", t.cli("boveda_vacia"));
        return Ok(());
    }
    println!("{}", t.cli("boveda_lista_cab"));
    for i in items {
        println!(
            "{} · {:>9} · {} · {}",
            i.id,
            size(i.meta.tamano),
            &i.meta.guardado[..16],
            i.meta.nombre
        );
    }
    Ok(())
}

fn sacar(t: &i18n::Texts, d: &Path, which: &str, dest: Option<&str>) -> Result<(), Box<dyn Error>> {
    let v = open(t, d)?;
    let item = v.find(which).map_err(|e| plain(t, &e))?;
    let out = dest.map_or_else(|| PathBuf::from(&item.meta.nombre), PathBuf::from);
    if out.exists() {
        println!(
            "{}",
            t.cli("boveda_existe_destino")
                .replace("{ruta}", &out.display().to_string())
        );
        return Ok(());
    }
    let mut f = std::fs::File::create(&out)?;
    let meta = match v.extract(&item.id, &mut f, QUIEN) {
        Ok(m) => m,
        Err(e) => {
            drop(f);
            let _ = std::fs::remove_file(&out);
            return Err(plain(t, &e).into());
        }
    };
    println!(
        "{}",
        t.cli("boveda_sacado")
            .replace("{ruta}", &out.display().to_string())
            .replace("{tamano}", &size(meta.tamano))
            .replace("{sha}", &meta.sha256)
    );
    Ok(())
}

fn registro(t: &i18n::Texts, d: &Path) -> Result<(), Box<dyn Error>> {
    let (entries, check) = Vault::log(d).map_err(|e| plain(t, &e))?;
    println!("{}", t.cli("boveda_registro_cab"));
    for e in &entries {
        println!(
            "{} · {} · {} · {} · {}",
            e.n,
            e.hora,
            e.accion,
            e.objeto.as_deref().unwrap_or("—"),
            e.detalle
        );
    }
    match check {
        Ok(n) => println!(
            "{}",
            t.cli("boveda_registro_ok").replace("{n}", &n.to_string())
        ),
        Err(n) => println!(
            "{}",
            t.cli("boveda_registro_rota").replace("{n}", &n.to_string())
        ),
    }
    Ok(())
}

fn ask_words(t: &i18n::Texts) -> Result<Vec<String>, Box<dyn Error>> {
    let s = line(t.cli("boveda_palabras_pide"))?;
    Ok(s.split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphabetic())
                .to_ascii_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect())
}

fn recuperar(t: &i18n::Texts, d: &Path) -> Result<(), Box<dyn Error>> {
    Vault::peek(d).map_err(|e| plain(t, &e))?;
    let w = ask_words(t)?;
    let refs: Vec<&str> = w.iter().map(String::as_str).collect();
    let mut v = Vault::open_with_words(d, &refs, QUIEN).map_err(|e| plain(t, &e))?;
    println!("{}", t.cli("boveda_recuperada"));
    let Some(p) = new_password(t)? else {
        return Ok(());
    };
    v.set_password(&p, QUIEN).map_err(|e| plain(t, &e))?;
    println!("{}", t.cli("boveda_contrasena_cambiada"));
    Ok(())
}

fn contrasena(t: &i18n::Texts, d: &Path) -> Result<(), Box<dyn Error>> {
    let mut v = open(t, d)?;
    let Some(p) = new_password(t)? else {
        return Ok(());
    };
    v.set_password(&p, QUIEN).map_err(|e| plain(t, &e))?;
    println!("{}", t.cli("boveda_contrasena_cambiada"));
    Ok(())
}

fn trozos(t: &i18n::Texts, d: &Path) -> Result<(), Box<dyn Error>> {
    let v = open(t, d)?;
    let w = ask_words(t)?;
    let refs: Vec<&str> = w.iter().map(String::as_str).collect();
    let pieces = v.pieces(&refs, QUIEN).map_err(|e| plain(t, &e))?;
    println!("\n{}\n", t.cli("boveda_trozos_titulo"));
    for p in pieces {
        println!("  {p}");
    }
    println!();
    Ok(())
}

fn juntar(t: &i18n::Texts) -> Result<(), Box<dyn Error>> {
    let mut texts = Vec::new();
    for i in 1..=vault::NEEDED {
        texts.push(line(
            &t.cli("boveda_trozo_pide").replace("{n}", &i.to_string()),
        )?);
    }
    let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    let w = vault::words_from_pieces(&refs).map_err(|e| plain(t, &e))?;
    println!("{}", t.cli("boveda_juntado"));
    print_words(t, &w);
    Ok(())
}

fn capsula(t: &i18n::Texts, d: &Path, dest: &Path) -> Result<(), Box<dyn Error>> {
    let v = open(t, d)?;
    let folder = v.capsule(dest, QUIEN).map_err(|e| plain(t, &e))?;
    println!(
        "{}",
        t.cli("boveda_capsula")
            .replace("{ruta}", &folder.display().to_string())
    );
    // The independent reader, if it was installed next to this program, travels with the capsule.
    let lector = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .map(|p| {
            p.join(if cfg!(windows) {
                "guardiana-lector.exe"
            } else {
                "guardiana-lector"
            })
        });
    match lector.filter(|p| p.exists()) {
        Some(p) => {
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            std::fs::copy(&p, folder.join(&name))?;
            println!(
                "{}",
                t.cli("boveda_capsula_lector").replace("{lector}", &name)
            );
        }
        None => println!("{}", t.cli("boveda_capsula_sin_lector")),
    }
    Ok(())
}

/// `guardiana boveda panel`: the vault's own page, served by this process on a loopback port until
/// the page says "Salir", half an hour idle, or Ctrl+C (decision 131).
fn panel(t: &i18n::Texts, d: &Path, abrir: bool) -> Result<(), Box<dyn Error>> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let s = guardiana_panel::boveda::bind(d.to_owned()).await?;
        let url = s.url();
        println!("{}", t.cli("boveda_panel_abriendo").replace("{url}", &url));
        if abrir {
            crate::engine::open_in_browser(&url);
        }
        s.run().await;
        Ok::<(), Box<dyn Error>>(())
    })
}

/// Entry point of `guardiana boveda`.
pub fn run(opts: &Opts) -> Result<(), Box<dyn Error>> {
    let t = i18n::current();
    let d = dir(opts);
    let words = opts.positional();
    match (words.first().map(String::as_str), words.get(1)) {
        (Some("crear"), _) => crear(t, &d),
        (Some("guardar"), Some(p)) => guardar(t, &d, Path::new(p)),
        (Some("lista"), _) => lista(t, &d),
        (Some("sacar"), Some(q)) => sacar(t, &d, q, opts.get("a")),
        (Some("registro"), _) => registro(t, &d),
        (Some("recuperar"), _) => recuperar(t, &d),
        (Some("contrasena" | "contraseña"), _) => contrasena(t, &d),
        (Some("trozos"), _) => trozos(t, &d),
        (Some("juntar"), _) => juntar(t),
        (Some("capsula" | "cápsula"), Some(p)) => capsula(t, &d, Path::new(p)),
        (Some("panel"), _) => panel(t, &d, !opts.has("no-open")),
        _ => {
            println!(
                "{}",
                t.cli("boveda_uso")
                    .replace("{ruta}", &default_dir().display().to_string())
            );
            Ok(())
        }
    }
}
