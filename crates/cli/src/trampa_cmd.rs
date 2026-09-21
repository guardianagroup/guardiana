//! `guardiana trampa crear | lista | quitar`: el cebo que dice quién lo mordió.
//!
//! Lo que hace y lo que no, en dos frases: escribe un archivo que lleva dentro una dirección
//! única y se queda esperando. Guardiana no mira ese archivo ni ningún otro —sigue mirando solo
//! nombres—; si alguna vez alguien pregunta por esa dirección, es que el archivo se leyó, y la
//! pregunta queda en el extracto con su hora y, en Windows, con el programa que la hizo.

use std::error::Error;
use std::path::PathBuf;

use guardiana_core::trampas::{self, Trampa};
use guardiana_core::{i18n, identity, paths, EventFilter, Ledger};

use crate::args::Opts;

pub fn run(opts: &Opts) -> Result<(), Box<dyn Error>> {
    let t = i18n::current();
    let mut l = Ledger::open(&paths::ledger_path(), identity::genesis())?;
    match opts.positional().first().map(String::as_str) {
        Some("crear") => crear(&mut l, opts),
        Some("quitar") => quitar(&mut l, opts),
        Some("lista") | None => lista(&l),
        Some(otro) => {
            eprintln!("{}", t.cli("trampa.uso").replace("{cmd}", otro));
            Err("".into())
        }
    }
}

fn crear(l: &mut Ledger, opts: &Opts) -> Result<(), Box<dyn Error>> {
    let t = i18n::current();
    let id = identificador();
    let carpeta: PathBuf = opts
        .get("carpeta")
        .or_else(|| opts.get("folder"))
        .map_or_else(escritorio, PathBuf::from);
    let nombre_archivo = opts
        .get("nombre")
        .or_else(|| opts.get("name"))
        .unwrap_or("claves-copia.txt");
    let archivo = carpeta.join(nombre_archivo);
    let trampa = Trampa {
        id,
        archivo: archivo.display().to_string(),
        creada: guardiana_core::time::now_ms(),
    };
    std::fs::create_dir_all(&carpeta)?;
    std::fs::write(&archivo, trampas::contenido(&trampa))?;
    let mut todas = trampas::listar(l)?;
    todas.push(trampa.clone());
    trampas::guardar(l, &todas)?;
    println!(
        "{}",
        t.cli("trampa.creada")
            .replace("{archivo}", &trampa.archivo)
            .replace("{nombre}", &trampa.nombre())
    );
    Ok(())
}

fn quitar(l: &mut Ledger, opts: &Opts) -> Result<(), Box<dyn Error>> {
    let t = i18n::current();
    let posicionales = opts.positional();
    let Some(id) = posicionales.get(1) else {
        eprintln!("{}", t.cli("trampa.falta_id"));
        return Err("".into());
    };
    let todas = trampas::listar(l)?;
    // Por identificador o por la ruta del archivo: la lista enseña las dos cosas y cualquiera de
    // las dos es lo que una persona copia. Antes solo valía el identificador, que la lista ni
    // siquiera imprimía: no había manera de acertar (visto en el repaso del 20 sep 2026).
    let Some(trampa) = todas
        .iter()
        .find(|x| &x.id == id || &x.archivo == id)
        .cloned()
    else {
        eprintln!("{}", t.cli("trampa.no_esta").replace("{id}", id));
        return Err("".into());
    };
    let _ = std::fs::remove_file(&trampa.archivo);
    let quedan: Vec<Trampa> = todas.into_iter().filter(|x| x.id != trampa.id).collect();
    trampas::guardar(l, &quedan)?;
    println!(
        "{}",
        t.cli("trampa.quitada")
            .replace("{archivo}", &trampa.archivo)
    );
    Ok(())
}

fn lista(l: &Ledger) -> Result<(), Box<dyn Error>> {
    let t = i18n::current();
    let todas = trampas::listar(l)?;
    if todas.is_empty() {
        println!("{}", t.cli("trampa.ninguna"));
        return Ok(());
    }
    for trampa in &todas {
        // ¿Alguien preguntó por su nombre? Eso es todo lo que hay que mirar: el extracto. Se le
        // pregunta por el nombre exacto, sin límite: una picada de hace meses cuenta igual.
        let nombre = trampa.nombre();
        let suyas = l.events(&EventFilter {
            qname: Some(nombre.clone()),
            ..EventFilter::default()
        })?;
        println!("{}  [{}]", trampa.archivo, trampa.id);
        println!("   {}", nombre);
        // Si alguien borró el archivo, la trampa ya no puede saltar. Decirlo, en vez de dejar
        // creer que sigue puesta: una protección que no existe es peor que ninguna.
        if !std::path::Path::new(&trampa.archivo).exists() {
            println!(
                "   {}",
                t.cli("trampa.sin_archivo").replace("{id}", &trampa.id)
            );
        }
        if let Some(e) = suyas.first() {
            let quien = e.process.as_ref().map_or_else(
                || t.cli("trampa.sin_programa").to_owned(),
                |p| p.nombre.clone(),
            );
            println!(
                "   {}",
                t.cli("trampa.picada")
                    .replace("{n}", &suyas.len().to_string())
                    .replace("{fecha}", &guardiana_core::time::rfc3339_utc(e.ts))
                    .replace("{quien}", &quien)
            );
        } else {
            println!("   {}", t.cli("trampa.intacta"));
        }
    }
    Ok(())
}

/// Ocho letras y números al azar, del mismo sitio de donde sale el token del panel.
fn identificador() -> String {
    let mut bytes = [0u8; 4];
    getrandom::fill(&mut bytes).unwrap_or_else(|_| {
        let t = guardiana_core::time::now_ms().to_le_bytes();
        bytes.copy_from_slice(&t[..4]);
    });
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// El escritorio de la persona, que es donde un archivo así se ve.
fn escritorio() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map_or_else(|| PathBuf::from("."), |h| PathBuf::from(h).join("Desktop"))
}
