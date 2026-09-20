//! `guardiana apps`: qué programa pidió cada nombre, en vivo (brief §12.3).
//!
//! Existe por dos razones. La primera es comprobar en la máquina lo que la documentación de
//! Microsoft promete: que el canal de sucesos del cliente DNS trae el nombre pedido y el proceso
//! que lo pidió, y con qué número de suceso. La segunda es que cualquiera pueda mirar, con sus
//! propios ojos y sin instalar nada más, lo que Guardiana va a anotar.
//!
//! No escribe en el extracto, no corta nada y no lee ningún contenido: enseña lo que el propio
//! Windows está contando de sí mismo.

use std::error::Error;
use std::time::{Duration, Instant};

use guardiana_core::i18n;

use crate::args::Opts;

pub fn run(opts: &Opts) -> Result<(), Box<dyn Error>> {
    let t = i18n::current();
    let segundos: u64 = opts
        .get("segundos")
        .or_else(|| opts.get("seconds"))
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);

    // `--crudo`: lo que el canal cuenta, sin filtrar, para comprobar los números de suceso en la
    // máquina antes de fiarse de la documentación (brief §12.3).
    if opts.has("crudo") || opts.has("raw") {
        let vistos = match guardiana_apps::diagnostico(segundos) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("{e}");
                return Err("".into());
            }
        };
        let mut por_id: std::collections::BTreeMap<u16, usize> = std::collections::BTreeMap::new();
        for (id, _, _) in &vistos {
            *por_id.entry(*id).or_default() += 1;
        }
        println!(
            "{}",
            t.cli("apps.crudo_total")
                .replace("{n}", &vistos.len().to_string())
        );
        for (id, n) in &por_id {
            println!("   suceso {id}: {n}");
        }
        for (id, nombre, pid) in vistos.iter().take(20) {
            if nombre.is_empty() {
                println!("   [{id}] pid {pid}");
            } else {
                println!("   [{id}] {nombre}  ←  pid {pid}");
            }
        }
        return Ok(());
    }

    let observador = match guardiana_apps::Observador::arrancar() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            return Err("".into());
        }
    };
    println!(
        "{}",
        t.cli("apps.escuchando")
            .replace("{s}", &segundos.to_string())
    );

    // Se pregunta por lo que Guardiana acaba de ver: el propio programa hace consultas para
    // comprobar listas y versión, así que hay tráfico real que mirar sin inventar nada.
    let hasta = Instant::now() + Duration::from_secs(segundos);
    let mut vistos = 0usize;
    while Instant::now() < hasta {
        std::thread::sleep(Duration::from_millis(250));
        let ahora = guardiana_core::time::now_ms();
        for nombre in nombres_recientes() {
            if let Some(p) = observador.quien_pidio(&nombre, ahora) {
                vistos += 1;
                let huella = if p.sha256.is_empty() {
                    "-".to_owned()
                } else {
                    p.sha256[..16.min(p.sha256.len())].to_owned()
                };
                println!("  {nombre}  ←  {} (pid {}, {huella}…)", p.nombre, p.pid);
                println!("     {}", p.ruta);
            }
        }
    }
    println!("{}", t.cli("apps.fin").replace("{n}", &vistos.to_string()));
    Ok(())
}

/// Los nombres que el equipo ha pedido hace nada, tomados del propio extracto: así lo que se
/// enseña es tráfico de verdad de esta máquina y no una lista escrita a mano.
fn nombres_recientes() -> Vec<String> {
    use guardiana_core::{identity, EventFilter, Ledger};
    let Ok(ledger) = Ledger::open(&guardiana_core::paths::ledger_path(), identity::genesis())
    else {
        return Vec::new();
    };
    let desde = guardiana_core::time::now_ms() - 10_000;
    let Ok(eventos) = ledger.events(&EventFilter {
        since: Some(desde),
        limit: Some(200),
        ..EventFilter::default()
    }) else {
        return Vec::new();
    };
    let mut fuera: Vec<String> = Vec::new();
    for e in eventos {
        if !fuera.contains(&e.qname) {
            fuera.push(e.qname);
        }
    }
    fuera
}
