//! Where the trial and the paid licence are anchored, besides the ledger.
//!
//! The seven days live in the ledger's `settings`, which lives with the person's own data.
//! Uninstalling does not touch that — measured on 22 September 2026: uninstall plus reinstall
//! keeps the same end date — but deleting the data folder starts the seven days again, and on
//! Windows the person at the console can delete it without being an administrator.
//!
//! So the date is written a second time, in a place only an administrator can remove: the
//! registry on Windows, a root-owned file on Linux and macOS. The earlier of the two wins, so the
//! trial can be neither restarted nor extended by touching one of them.
//!
//! Since the review of 1 Oct 2026 (entries 9, 11, 12 and 24) the same place holds two more
//! things, next to the trial mark and read back when the ledger lacks them: the latest clock
//! reading the program has seen, so moving the clock back does not stretch the trial, and what
//! it takes to get the paid licence back — the key, the number of this activation and the
//! product — so a lost data folder does not turn a paying customer into "trial over". Not the
//! customer's name nor e-mail, and not the licence's secret: on Windows any user of the machine
//! can read HKLM, and on Linux the file outlives `apt purge` (review of 5 Oct 2026, licence item
//! 4). The trial mark itself is unchanged: the earlier of ledger and anchor still wins.
//!
//! **Nothing here is hidden.** The panel says where the mark is and what it holds, and
//! `guardiana verify` prints it. A program that asks to be checked cannot leave marks it does not
//! talk about. And every write says whether it happened: without administrator rights it does
//! not, and the interface must not claim otherwise (entry 24).
//!
//! With `GUARDIANA_DATA` set (test instances and development) the marks live inside that folder
//! instead, so trying the program out never touches the machine's own. Not when the variable
//! names the machine's own data folder: a Mac daemon installed with it set kept its marks next
//! to the data they protect (review of 5 Oct 2026, licence item 5).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[cfg(windows)]
const CLAVE: &str = r"HKLM\SOFTWARE\Guardiana";

/// Each value, by its name in the registry and its file name on Linux, macOS and in the test
/// folder. The trial mark keeps the names 1.0.0 gave it.
const PRUEBA: (&str, &str) = ("prueba", "prueba-empezada");
const VISTO: (&str, &str) = ("visto", "visto-hasta");
const LICENCIA: (&str, &str) = ("licencia", "licencia");
const VALORES: [(&str, &str); 3] = [PRUEBA, VISTO, LICENCIA];

/// On Windows the licence goes into the registry through `reg add`, whose command line has a
/// limit of 32 K characters; a value this long is not ours and is not written.
#[cfg(windows)]
const MAXIMO_REGISTRO: usize = 30_000;

/// The paid licence as the anchor keeps it: enough to put it back into a ledger that lost it and
/// to ask the gateway about it (review of 1 Oct 2026, entries 9 and 12), and nothing about the
/// person (review of 5 Oct 2026, licence item 4). Kept where only an administrator can remove
/// it, and the panel says so.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Licencia {
    /// The activated key.
    #[serde(default)]
    pub clave: String,
    /// The gateway's id for this activation (`license_key_instance_id`).
    #[serde(default)]
    pub instancia: String,
    /// The gateway's product id, which decides the billing period. Empty if unknown.
    #[serde(default)]
    pub producto: String,
    /// Unix ms of the activation.
    #[serde(default)]
    pub desde: i64,
    /// Days per billing period (30, 365, or 0 for a licence bought once).
    #[serde(default)]
    pub periodo: i64,
}

/// Everything the anchor holds, read in one go.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Marcas {
    /// When the trial started, Unix ms.
    pub prueba: Option<i64>,
    /// The latest clock reading seen, Unix ms.
    pub visto: Option<i64>,
    /// The paid licence, if one was activated.
    pub licencia: Option<Licencia>,
}

/// Where the trial mark lives, in words, for the panel and for `verify`. The other values live
/// next to it, under the same key or in the same folder.
#[must_use]
pub fn donde() -> String {
    if let Some(d) = carpeta_de_pruebas() {
        return d.join(PRUEBA.1).display().to_string();
    }
    #[cfg(windows)]
    {
        format!(r"{CLAVE}\{}", PRUEBA.0)
    }
    #[cfg(not(windows))]
    {
        carpeta_sistema().join(PRUEBA.1).display().to_string()
    }
}

/// Where the licence copy lives, in words, for the panel.
#[must_use]
pub fn donde_licencia() -> String {
    if let Some(d) = carpeta_de_pruebas() {
        return d.join(LICENCIA.1).display().to_string();
    }
    #[cfg(windows)]
    {
        format!(r"{CLAVE}\{}", LICENCIA.0)
    }
    #[cfg(not(windows))]
    {
        carpeta_sistema().join(LICENCIA.1).display().to_string()
    }
}

/// A folder of its own for the marks, set by a program that is not the service: GUARDIANA ZERO
/// runs as the person, without administrator rights, and has seven days of its own (decided by
/// the owner on 9 Oct 2026), so its marks must neither read nor touch the service's. `None`
/// (the default, and what the service always has) keeps the places described above.
static CARPETA_PROPIA: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

/// Keep this process's marks in `dir` (or back in the usual places with `None`). Called once,
/// before the first licence call, by a program that is not the service.
pub fn usar_carpeta(dir: Option<PathBuf>) {
    *CARPETA_PROPIA
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = dir;
}

/// In test or development mode (`GUARDIANA_DATA`), the marks go with the data. When the variable
/// names the machine's own data folder it is not a test: the marks stay where they belong.
fn carpeta_de_pruebas() -> Option<PathBuf> {
    if let Some(dir) = CARPETA_PROPIA
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
    {
        return Some(dir);
    }
    let dir = PathBuf::from(std::env::var_os(guardiana_core::paths::DATA_ENV)?);
    if misma_carpeta(&dir, &guardiana_core::paths::system_data_dir()) {
        return None;
    }
    Some(dir)
}

/// Whether two paths name the same folder, resolving links when both exist.
fn misma_carpeta(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => a.components().eq(b.components()),
    }
}

/// Fuera de la carpeta de datos, y donde solo manda root: en macOS los datos viven justamente
/// en `/Library/Application Support/Guardiana`, así que poner ahí la marca no serviría de nada
/// —se borraría con ellos— y además la escribiría cualquier usuario. `/etc` existe en los dos
/// sistemas y solo lo toca un administrador.
#[cfg(not(windows))]
fn carpeta_sistema() -> PathBuf {
    PathBuf::from("/etc/guardiana")
}

fn leer_archivos(dir: &Path) -> Vec<(&'static str, String)> {
    VALORES
        .iter()
        .filter_map(|(nombre, archivo)| {
            std::fs::read_to_string(dir.join(archivo))
                .ok()
                .map(|v| (*nombre, v))
        })
        .collect()
}

/// Write one value as a file. The licence is readable by its owner only: it holds the key.
fn escribir_archivo(p: &Path, texto: &str, privado: bool) -> bool {
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    #[cfg(unix)]
    if privado {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        return std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(p)
            .and_then(|mut f| f.write_all(texto.as_bytes()))
            .is_ok();
    }
    #[cfg(not(unix))]
    let _ = privado;
    std::fs::write(p, texto).is_ok()
}

fn escribir_en_carpeta(dir: &Path, nombre: &str, valor: &str) -> bool {
    let Some((_, archivo)) = VALORES.iter().find(|(n, _)| *n == nombre) else {
        return false;
    };
    escribir_archivo(&dir.join(archivo), valor, nombre == LICENCIA.0)
}

/// Every value the anchor holds, as plain text.
fn leer_valores() -> Vec<(&'static str, String)> {
    #[cfg(windows)]
    {
        match carpeta_de_pruebas() {
            Some(dir) => leer_archivos(&dir),
            None => registro_leer(),
        }
    }
    #[cfg(not(windows))]
    {
        leer_archivos(&carpeta_de_pruebas().unwrap_or_else(carpeta_sistema))
    }
}

/// Write one value. `true` only when it is really there afterwards: without administrator
/// rights it is not, and the caller must not claim otherwise.
fn escribir_valor(nombre: &str, valor: &str) -> bool {
    #[cfg(windows)]
    {
        match carpeta_de_pruebas() {
            Some(dir) => escribir_en_carpeta(&dir, nombre, valor),
            None => registro_escribir(nombre, valor),
        }
    }
    #[cfg(not(windows))]
    {
        escribir_en_carpeta(
            &carpeta_de_pruebas().unwrap_or_else(carpeta_sistema),
            nombre,
            valor,
        )
    }
}

/// `reg` instead of a crate: one more dependency for a few lines is not worth it, and the
/// service runs as SYSTEM, which can read and write HKLM. One query for the whole key, so a
/// status call costs one process, not three.
#[cfg(windows)]
fn registro_leer() -> Vec<(&'static str, String)> {
    let Ok(salida) = std::process::Command::new("reg")
        .args(["query", CLAVE])
        .output()
    else {
        return Vec::new();
    };
    if !salida.status.success() {
        return Vec::new();
    }
    let texto = String::from_utf8_lossy(&salida.stdout);
    let mut valores = Vec::new();
    for linea in texto.lines() {
        let mut partes = linea.split_whitespace();
        let (Some(nombre), Some(tipo), Some(valor)) = (partes.next(), partes.next(), partes.next())
        else {
            continue;
        };
        if !tipo.starts_with("REG_") {
            continue;
        }
        let Some((n, _)) = VALORES.iter().find(|(n, _)| *n == nombre) else {
            continue;
        };
        // The licence is JSON with quotes and spaces, which a command line would mangle: it
        // travels as hex (see `registro_escribir`).
        let valor = if *n == LICENCIA.0 {
            des_hex(valor).unwrap_or_default()
        } else {
            valor.to_owned()
        };
        valores.push((*n, valor));
    }
    valores
}

#[cfg(windows)]
fn registro_escribir(nombre: &str, valor: &str) -> bool {
    let dato = if nombre == LICENCIA.0 {
        a_hex(valor)
    } else {
        valor.to_owned()
    };
    if dato.len() > MAXIMO_REGISTRO {
        return false;
    }
    std::process::Command::new("reg")
        .args([
            "add", CLAVE, "/v", nombre, "/t", "REG_SZ", "/d", &dato, "/f",
        ])
        .output()
        .is_ok_and(|s| s.status.success())
}

#[cfg(any(windows, test))]
fn a_hex(texto: &str) -> String {
    texto.bytes().map(|b| format!("{b:02x}")).collect()
}

#[cfg(any(windows, test))]
fn des_hex(hex: &str) -> Option<String> {
    let hex = hex.trim();
    if hex.len() % 2 != 0 {
        return None;
    }
    let bytes = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok())
        .collect::<Option<Vec<u8>>>()?;
    String::from_utf8(bytes).ok()
}

/// Everything the anchor holds. Missing or unreadable values are `None`.
#[must_use]
pub fn leer_todo() -> Marcas {
    let valores = leer_valores();
    let texto = |nombre: &str| {
        valores
            .iter()
            .find(|(n, _)| *n == nombre)
            .map(|(_, v)| v.as_str())
    };
    let numero = |nombre: &str| texto(nombre).and_then(|v| v.trim().parse::<i64>().ok());
    Marcas {
        prueba: numero(PRUEBA.0),
        visto: numero(VISTO.0),
        // A copy written by an unpublished 1.0.2 build (secret and full activation, no instance
        // field) does not parse into one with an instance, and is rewritten from the ledger.
        licencia: texto(LICENCIA.0)
            .and_then(|v| serde_json::from_str::<Licencia>(v).ok())
            .filter(|l| !l.clave.is_empty() && !l.instancia.is_empty()),
    }
}

/// When the trial started, according to the mark. `None` if there is none or it cannot be read.
#[must_use]
pub fn leer() -> Option<i64> {
    leer_todo().prueba
}

/// Write the trial mark. `true` when it was written; without administrator rights it is not,
/// the trial still works with what the ledger says, and the interface says the mark is missing
/// instead of claiming it is there (review of 1 Oct 2026, entry 24).
pub fn escribir(ms: i64) -> bool {
    escribir_valor(PRUEBA.0, &ms.to_string())
}

/// Write the latest clock reading seen. Same best effort, same honest answer.
pub fn escribir_visto(ms: i64) -> bool {
    escribir_valor(VISTO.0, &ms.to_string())
}

/// Write the copy of the paid licence. Same best effort, same honest answer.
pub fn escribir_licencia(licencia: &Licencia) -> bool {
    serde_json::to_string(licencia).is_ok_and(|texto| escribir_valor(LICENCIA.0, &texto))
}

/// Las pruebas del crate corren en paralelo dentro del mismo proceso y `GUARDIANA_DATA` es de
/// todo el proceso: sin cerrojo, la prueba que escribe la marca se la cambia a las demás, y la
/// que no lo sepa acabará escribiendo en la carpeta de datos de verdad de esta máquina.
#[cfg(test)]
pub(crate) static CERROJO: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Toma el cerrojo y deja `GUARDIANA_DATA` apuntando a una carpeta propia de la prueba, vacía:
/// lo que dejó una pasada anterior (la carpeta lleva el pid, que el sistema reutiliza) no puede
/// colarse en esta.
#[cfg(test)]
pub(crate) fn a_solas_en(dir: &std::path::Path) -> std::sync::MutexGuard<'static, ()> {
    let g = CERROJO
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).ok();
    std::env::set_var("GUARDIANA_DATA", dir);
    g
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn carpeta(nombre: &str) -> PathBuf {
        std::env::temp_dir().join(format!("guardiana-ancla-{nombre}-{}", std::process::id()))
    }

    /// With `GUARDIANA_DATA` the mark goes into that folder and nowhere else: probar el programa
    /// no deja nada en la máquina de quien lo prueba.
    #[test]
    fn con_carpeta_de_pruebas_la_marca_va_dentro() {
        let dir = carpeta("prueba");
        let _a_solas = a_solas_en(&dir);
        assert!(leer().is_none(), "empieza sin marca");
        assert!(escribir(1_790_000_000_000), "se escribió");
        assert_eq!(leer(), Some(1_790_000_000_000));
        assert!(donde().contains("prueba-empezada"));
        assert!(donde().starts_with(&dir.display().to_string()));
        std::env::remove_var("GUARDIANA_DATA");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A program with a folder of its own (GUARDIANA ZERO) keeps its marks there and never sees
    /// the service's; clearing it gives the usual places back.
    #[test]
    fn una_carpeta_propia_aparta_las_marcas_del_servicio() {
        let servicio = carpeta("servicio");
        let propia = carpeta("propia");
        let _a_solas = a_solas_en(&servicio);
        assert!(escribir(1_790_000_000_000), "marca del servicio");
        let _ = std::fs::remove_dir_all(&propia);
        usar_carpeta(Some(propia.clone()));
        assert!(
            leer().is_none(),
            "la marca del servicio no se ve desde la carpeta propia"
        );
        assert!(escribir(1_791_000_000_000));
        assert_eq!(leer(), Some(1_791_000_000_000));
        assert!(donde().starts_with(&propia.display().to_string()));
        usar_carpeta(None);
        assert_eq!(
            leer(),
            Some(1_790_000_000_000),
            "la del servicio sigue intacta"
        );
        std::env::remove_var("GUARDIANA_DATA");
        std::fs::remove_dir_all(&servicio).ok();
        std::fs::remove_dir_all(&propia).ok();
    }

    /// The clock mark and the licence copy go next to the trial mark and come back whole, and
    /// none of the three disturbs the other two (review of 1 Oct 2026, entries 11 and 12).
    #[test]
    fn el_reloj_y_la_licencia_van_junto_a_la_marca_y_vuelven_enteros() {
        let dir = carpeta("licencia");
        let _a_solas = a_solas_en(&dir);
        assert_eq!(leer_todo(), Marcas::default());
        let licencia = Licencia {
            clave: "KEY-1".to_owned(),
            instancia: "lki_1".to_owned(),
            producto: "pdt_x".to_owned(),
            desde: 5,
            periodo: 30,
        };
        assert!(escribir(1));
        assert!(escribir_visto(2));
        assert!(escribir_licencia(&licencia));
        let m = leer_todo();
        assert_eq!(m.prueba, Some(1));
        assert_eq!(m.visto, Some(2));
        assert_eq!(m.licencia, Some(licencia.clone()));
        // A newer clock reading overwrites the old one and leaves the rest alone.
        assert!(escribir_visto(3));
        let m = leer_todo();
        assert_eq!(m.prueba, Some(1));
        assert_eq!(m.visto, Some(3));
        assert_eq!(m.licencia, Some(licencia));
        // On Linux and macOS the licence file is the owner's alone: it holds the key.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let modo = std::fs::metadata(dir.join(LICENCIA.1))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(modo & 0o777, 0o600, "{modo:o}");
        }
        std::env::remove_var("GUARDIANA_DATA");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// What travels to the Windows registry as hex comes back as it went, quotes and accents
    /// included; and garbage does not pass for a licence.
    #[test]
    fn el_hex_del_registro_va_y_vuelve() {
        let texto = r#"{"clave":"K","instancia":"lki \"1\"","producto":"Pérez"}"#;
        assert_eq!(des_hex(&a_hex(texto)).as_deref(), Some(texto));
        assert_eq!(des_hex("abc"), None, "longitud impar");
        assert_eq!(des_hex("zz"), None, "no es hex");
        assert_eq!(des_hex(""), Some(String::new()));
    }
}
