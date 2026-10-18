//! «Sala de entrega» on this computer (18 Oct 2026): the files go in from the page, are sealed
//! piece by piece on the disk ([`guardiana_secure::flujo`]), and wait for the one receiver who
//! has the link and the code. The rules (expiry, one use, the code's tries, the receipt's text)
//! are in [`guardiana_secure::entrega`]; this module carries the bytes and keeps the state.
//!
//! The road, end to end:
//! 1. The page hands each file over in pieces; each 1 MiB is sealed with that file's own
//!    AES-256-GCM key in the STREAM construction and written to `entregas/<id>/f<i>.bin`. What
//!    lies on the disk is ciphertext; the keys live in `estado.bin`, sealed with a key that
//!    DPAPI protects for this Windows account ([`crate::dpapi`]).
//! 2. «Crear enlace» makes a 256-bit link key. The link (`guardianagroup.com/e/#…`) carries it
//!    in the fragment, which a browser never sends; the room at the relay is a hash of it.
//! 3. This computer waits in that room. The receiver's page meets it there with the phone's
//!    greeting (`movil`'s «GM2»: a fresh ECDH P-256 secret mixed with the link key, HKDF-SHA256,
//!    AES-256-GCM with counters both ways), so the relay passes only ciphertext and cannot
//!    replay, reorder or change a frame.
//! 4. Inside, the page gives the code. Only after the right code are the files' keys sent; the
//!    page then asks for pieces, a few at a time, from where it is (a resumed transfer first
//!    checks the root of the hash tree of what it holds).
//! 5. The page confirms each file with its tree root, signs the receipt's text with a fresh
//!    ECDSA P-256 key of its own (WebCrypto), and this computer adds its signature
//!    ([`crate::recibo`]): both sides keep the same signed receipt. The events go into the
//!    deliveries' own hash chain, signed with this computer's identity when exported.
//!
//! Never written anywhere: an address (this computer only ever talks to the relay), the code
//! outside the sealed state, a file's contents in the clear.

use std::collections::VecDeque;
use std::fs::File;
use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use guardiana_secure::entrega::{Caducidad, Entrega, Estado, Ficha, Intento};
use guardiana_secure::flujo::{self, Clave, Empaquetador, TROZO};
use guardiana_secure::monitor::Nivel;
use guardiana_secure::prueba::{Cadena, Hecho};
use guardiana_secure::{arbol, relleno};
use ring::aead;
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::sync::mpsc as tmpsc;

use crate::movil;

/// Where the receiver's page lives (static; it never sees a file or a key).
pub const PAGINA: &str = "https://guardianagroup.com/e/";
const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
const B64URL: base64::engine::GeneralPurpose = base64::engine::general_purpose::URL_SAFE_NO_PAD;
/// Bytes of one stored piece: its length, the sealed piece, the tag.
const PASO: u64 = 4 + TROZO as u64 + 16;
/// Pieces the page may ask for at once.
const PIDE_MAX: u64 = 8;
/// How long to wait for the receiver's own signature before signing the receipt alone.
const ACUSE_S: u64 = 120;
/// Deliveries kept on the screen (finished ones included) at most.
const MAX_ENTREGAS: usize = 50;

fn ahora_s() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// The room at the relay: a hash of the link key under this room's own label.
#[must_use]
pub fn sala_de(clave: &[u8; 32]) -> String {
    let h = Sha256::new()
        .chain_update(b"guardiana-entrega-sala")
        .chain_update(clave)
        .finalize();
    hex(&h[..16])
}

/// What the receiver's thread tells this computer.
#[derive(Debug)]
enum DelHilo {
    /// A page greeted through the relay.
    Conectado,
    /// It went away (it may come back with the same link).
    Soltado,
    /// It gives the code, with the token its tab keeps (empty the first time).
    Codigo(String, String),
    /// It holds this many verified pieces in all, and this many failed.
    Verificados(u64, u64),
    /// A piece went out.
    Enviado(u64),
    /// File `i` is whole there, with this tree root.
    Recibido(usize, String),
    /// Its signature over the receipt's text: public key and signature, base64.
    Acuse(String, String),
    /// A frame did not open: tampered with on the way.
    Manipulado,
}

/// What this computer tells the receiver's thread.
#[derive(Debug)]
enum AlHilo {
    /// The right code: the files and their keys (JSON for the page).
    Bien(String),
    /// A wrong code, this many tries left.
    Mal(u32),
    /// The receipt's text, for the page to sign.
    Recibo(String),
    /// The receipt with both signatures (JSON).
    ReciboFinal(String),
    /// The end, with its reason (`entregada`, `caducada`, `revocada`, `destruida`…).
    Fin(String),
}

/// What the thread needs to serve the pieces.
struct Datos {
    carpeta: PathBuf,
    /// Per file: its pieces and their tree's leaves.
    hojas: Vec<Arc<Vec<[u8; 32]>>>,
}

struct Hilo {
    tx: tmpsc::UnboundedSender<AlHilo>,
    rx: Receiver<DelHilo>,
}

/// A file being sealed as the page hands it over.
struct Carga {
    empaquetador: Empaquetador,
    clave: Clave,
    archivo: File,
    nombre: String,
    escritos: u64,
}

/// One delivery: its rules, its keys, its thread.
struct Sala {
    e: Entrega,
    claves: Vec<Clave>,
    hojas: Vec<Arc<Vec<[u8; 32]>>>,
    enlace: Option<[u8; 32]>,
    carga: Option<Carga>,
    hilo: Option<Hilo>,
    /// The receipt's text waiting for the receiver's signature, and since when.
    acuse: Option<(String, u64)>,
    recibo: Option<crate::recibo::Recibo>,
    conectado: bool,
    enviados: u64,
    verificados: u64,
    fallidos: u64,
}

/// The deliveries of this computer.
pub struct Entregas {
    carpeta: PathBuf,
    rele: String,
    clave_estado: [u8; 32],
    clave_recibos: Option<Vec<u8>>,
    salas: Vec<Sala>,
    /// The deliveries' hash chain (what happened, never a file or an address).
    pub cadena: Cadena,
    avisos: VecDeque<Value>,
    cambio: bool,
}

impl std::fmt::Debug for Entregas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Entregas")
            .field("salas", &self.salas.len())
            .finish()
    }
}

fn sella_estado(clave: &[u8; 32], plano: &[u8]) -> Option<Vec<u8>> {
    let mut n = [0u8; 12];
    crate::sesion::azar(&mut n);
    let k = aead::LessSafeKey::new(aead::UnboundKey::new(&aead::AES_256_GCM, clave).ok()?);
    let mut v = plano.to_vec();
    k.seal_in_place_append_tag(
        aead::Nonce::assume_unique_for_key(n),
        aead::Aad::from(b"guardiana-entrega-estado"),
        &mut v,
    )
    .ok()?;
    let mut o = n.to_vec();
    o.extend(v);
    Some(o)
}

fn abre_estado(clave: &[u8; 32], sellado: &[u8]) -> Option<Vec<u8>> {
    let (n, resto) = sellado.split_first_chunk::<12>()?;
    let k = aead::LessSafeKey::new(aead::UnboundKey::new(&aead::AES_256_GCM, clave).ok()?);
    let mut v = resto.to_vec();
    let largo = k
        .open_in_place(
            aead::Nonce::assume_unique_for_key(*n),
            aead::Aad::from(b"guardiana-entrega-estado"),
            &mut v,
        )
        .ok()?
        .len();
    v.truncate(largo);
    Some(v)
}

/// A name the page gave, safe as a relative path inside the receiver's folder: no drive, no
/// `..`, no characters Windows refuses.
fn ruta_limpia(n: &str) -> String {
    let partes: Vec<String> = n
        .split(['/', '\\'])
        .map(|p| {
            p.chars()
                .filter(|c| {
                    !matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|') && !c.is_control()
                })
                .take(120)
                .collect::<String>()
                .trim()
                .trim_end_matches(['.', ' '])
                .to_owned()
        })
        .filter(|p| !p.is_empty() && p != "." && p != "..")
        .take(16)
        .collect();
    if partes.is_empty() {
        "archivo".to_owned()
    } else {
        partes.join("/")
    }
}

impl Sala {
    fn guarda(&self, carpeta: &Path, clave: &[u8; 32]) {
        let claves: Vec<Value> = self
            .claves
            .iter()
            .map(|c| json!({ "k": B64.encode(c.bytes()), "p": B64.encode(c.prefijo()) }))
            .collect();
        let v = json!({
            "entrega": self.e,
            "claves": claves,
            "enlace": self.enlace.map(|k| B64.encode(k)),
            "recibo": self.recibo,
        });
        let dir = carpeta.join(&self.e.id);
        let _ = std::fs::create_dir_all(&dir);
        if let Some(s) = sella_estado(clave, v.to_string().as_bytes()) {
            let _ = std::fs::write(dir.join("estado.bin"), s);
        }
    }

    fn carga_de(dir: &Path, clave: &[u8; 32]) -> Option<Self> {
        let v: Value = serde_json::from_slice(&abre_estado(
            clave,
            &std::fs::read(dir.join("estado.bin")).ok()?,
        )?)
        .ok()?;
        let e: Entrega = serde_json::from_value(v["entrega"].clone()).ok()?;
        let mut claves = Vec::new();
        for c in v["claves"].as_array()? {
            let k: [u8; 32] = B64.decode(c["k"].as_str()?).ok()?.try_into().ok()?;
            let p: [u8; flujo::PREFIJO] = B64.decode(c["p"].as_str()?).ok()?.try_into().ok()?;
            claves.push(Clave::de(k, p));
        }
        let enlace = v["enlace"]
            .as_str()
            .and_then(|s| B64.decode(s).ok())
            .and_then(|b| <[u8; 32]>::try_from(b.as_slice()).ok());
        let hojas = (0..e.archivos.len())
            .map(|i| {
                let b = std::fs::read(dir.join(format!("f{i}.hojas"))).unwrap_or_default();
                Arc::new(
                    b.chunks_exact(32)
                        .filter_map(|c| <[u8; 32]>::try_from(c).ok())
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        Some(Self {
            recibo: serde_json::from_value(v["recibo"].clone()).ok(),
            e,
            claves,
            hojas,
            enlace,
            carga: None,
            hilo: None,
            acuse: None,
            conectado: false,
            enviados: 0,
            verificados: 0,
            fallidos: 0,
        })
    }

    /// The keys and the sealed files go: nothing of the delivery can be opened any more. The
    /// state (with the receipt, if any) stays for the screen until the person deletes it.
    fn borra_contenido(&mut self, carpeta: &Path) {
        self.claves.clear();
        self.enlace = None;
        self.carga = None;
        if let Some(h) = self.hilo.take() {
            let _ = h.tx.send(AlHilo::Fin(self.e.estado.nombre().to_owned()));
        }
        let dir = carpeta.join(&self.e.id);
        for i in 0..self.e.archivos.len() {
            let _ = std::fs::remove_file(dir.join(format!("f{i}.bin")));
        }
        self.conectado = false;
    }

    fn enlace_texto(&self, rele: &str, idioma: &str) -> Option<String> {
        let k = self.enlace?;
        if !self.e.estado.viva() {
            return None;
        }
        let rele = rele.strip_suffix(":443").unwrap_or(rele);
        let d = json!({ "v": 1, "k": B64URL.encode(k), "r": rele, "i": self.e.id, "l": idioma });
        Some(format!("{PAGINA}#{}", B64URL.encode(d.to_string())))
    }

    /// The files and their keys, for the receiver who gave the right code.
    fn manifiesto(&self, token: &str) -> String {
        let archivos: Vec<Value> = self
            .e
            .archivos
            .iter()
            .zip(&self.claves)
            .enumerate()
            .map(|(i, (a, c))| {
                json!({
                    "i": i, "nombre": a.nombre, "bytes": a.bytes, "trozos": a.trozos,
                    "sha256": a.sha256, "raiz": a.raiz,
                    "clave": B64.encode(c.bytes()), "prefijo": B64.encode(c.prefijo()),
                })
            })
            .collect();
        json!({
            "t": "manifiesto", "token": token, "entrega": self.e.id, "trozo": TROZO,
            "vence": self.e.vence, "caducidad": self.e.caducidad.nombre(), "archivos": archivos,
        })
        .to_string()
    }
}

impl Entregas {
    /// Open the deliveries kept under `datos/secure/entregas`, and go back to waiting for the
    /// receivers of those still open.
    #[must_use]
    pub fn abre(datos: &Path) -> Self {
        Self::abre_con(datos, &crate::rele::direccion())
    }

    /// The same, with the relay at `rele` (`host:port`; the tests run one here).
    #[must_use]
    pub fn abre_con(datos: &Path, rele: &str) -> Self {
        let carpeta = datos.join("secure").join("entregas");
        let _ = std::fs::create_dir_all(&carpeta);
        let clave_estado = crate::dpapi::lee_o_crea(
            &carpeta.join("clave.bin"),
            || {
                let mut k = [0u8; 32];
                crate::sesion::azar(&mut k);
                Some(k.to_vec())
            },
            |b| b.len() == 32,
        )
        .and_then(|b| <[u8; 32]>::try_from(b.as_slice()).ok())
        .unwrap_or([0u8; 32]);
        let clave_recibos = crate::dpapi::lee_o_crea(
            &carpeta.join("clave-recibos.p8"),
            || crate::recibo::clave_nueva().ok(),
            |b| crate::recibo::huella(b).is_ok(),
        );
        let cadena = std::fs::read_to_string(carpeta.join("libro.json"))
            .ok()
            .and_then(|s| serde_json::from_str::<Cadena>(&s).ok())
            .filter(|c| c.comprueba().is_ok())
            .unwrap_or_else(|| Cadena::new("entregas", ahora_s()));
        let mut salas: Vec<Sala> = std::fs::read_dir(&carpeta)
            .map(|d| {
                d.filter_map(Result::ok)
                    .filter(|x| x.path().is_dir())
                    .filter_map(|x| Sala::carga_de(&x.path(), &clave_estado))
                    .collect()
            })
            .unwrap_or_default();
        salas.sort_by_key(|s| s.e.creada);
        let mut e = Self {
            carpeta,
            rele: rele.to_owned(),
            clave_estado,
            clave_recibos,
            salas,
            cadena,
            avisos: VecDeque::new(),
            cambio: true,
        };
        for i in 0..e.salas.len() {
            if e.salas[i].e.estado == Estado::Preparando {
                // Files were going in when the program closed: that delivery cannot be finished.
                e.salas[i].e.revoca();
            }
            if matches!(e.salas[i].e.estado, Estado::Esperando | Estado::Abierta) {
                e.arranca(i);
            }
        }
        e
    }

    fn anota(&mut self, texto: String) {
        self.cadena.anota(
            ahora_s(),
            Nivel::Info,
            Hecho {
                texto,
                evidencia: Some("GUARDIANA · sala de entrega".to_owned()),
                ..Hecho::default()
            },
        );
        let _ = std::fs::write(
            self.carpeta.join("libro.json"),
            serde_json::to_string(&self.cadena).unwrap_or_default(),
        );
    }

    fn guarda(&self, i: usize) {
        if let Some(s) = self.salas.get(i) {
            s.guarda(&self.carpeta, &self.clave_estado);
        }
    }

    fn indice(&self, id: &str) -> Option<usize> {
        self.salas.iter().position(|s| s.e.id == id)
    }

    /// A new delivery, empty, for the files the page is about to hand over. Returns its id.
    pub fn nueva(
        &mut self,
        caducidad: &str,
        digitos: usize,
        nombres: bool,
        titulo: &str,
    ) -> Result<String, String> {
        // A delivery left half-filled is withdrawn: only one fills at a time.
        let a_medias: Vec<usize> = (0..self.salas.len())
            .filter(|&i| self.salas[i].e.estado == Estado::Preparando)
            .collect();
        for i in a_medias {
            self.salas[i].e.revoca();
            self.salas[i].borra_contenido(&self.carpeta);
            self.guarda(i);
        }
        while self.salas.len() >= MAX_ENTREGAS {
            let Some(i) = self.salas.iter().position(|s| !s.e.estado.viva()) else {
                return Err("sec_entrega_demasiadas".to_owned());
            };
            self.borra(i);
        }
        let mut e = Entrega::nueva(ahora_s(), Caducidad::de(caducidad), digitos, nombres)
            .map_err(|e| e.to_string())?;
        e.titulo = titulo.chars().take(80).collect();
        let id = e.id.clone();
        let _ = std::fs::create_dir_all(self.carpeta.join(&id));
        self.salas.push(Sala {
            e,
            claves: Vec::new(),
            hojas: Vec::new(),
            enlace: None,
            carga: None,
            hilo: None,
            acuse: None,
            recibo: None,
            conectado: false,
            enviados: 0,
            verificados: 0,
            fallidos: 0,
        });
        self.anota(format!("entrega {id} creada"));
        self.cambio = true;
        Ok(id)
    }

    fn preparando(&mut self) -> Result<usize, String> {
        self.salas
            .iter()
            .position(|s| s.e.estado == Estado::Preparando)
            .ok_or_else(|| "sec_entrega_no_hay".to_owned())
    }

    /// A file starts: its name (with its folder) and its size as the page knows it.
    pub fn archivo(&mut self, nombre: &str, bytes: u64) -> Result<(), String> {
        if bytes > guardiana_secure::archivo::MAX_BYTES {
            return Err("sec_archivo_grande".to_owned());
        }
        let i = self.preparando()?;
        let s = &mut self.salas[i];
        if s.carga.is_some() {
            return Err("sec_entrega_a_medias".to_owned());
        }
        let n = s.e.archivos.len();
        let clave = Clave::nueva().map_err(|e| e.to_string())?;
        let empaquetador = Empaquetador::new(&clave, flujo::contexto(&s.e.id, n as u32))
            .map_err(|e| e.to_string())?;
        let archivo = File::create(self.carpeta.join(&s.e.id).join(format!("f{n}.bin")))
            .map_err(|e| e.to_string())?;
        s.carga = Some(Carga {
            empaquetador,
            clave,
            archivo,
            nombre: ruta_limpia(nombre),
            escritos: 0,
        });
        Ok(())
    }

    fn escribe_trozos(
        archivo: &mut File,
        escritos: &mut u64,
        sellados: Vec<Vec<u8>>,
    ) -> Result<(), String> {
        for t in sellados {
            let mut registro = (t.len() as u32).to_be_bytes().to_vec();
            registro.extend_from_slice(&t);
            registro.resize(PASO as usize, 0);
            archivo
                .write_all(&registro)
                .map_err(|_| "sec_err_archivo".to_owned())?;
            *escritos += 1;
        }
        Ok(())
    }

    /// More bytes of the current file; `ultimo` closes it.
    pub fn trozo(&mut self, datos: &[u8], ultimo: bool) -> Result<(), String> {
        let i = self.preparando()?;
        let carpeta = self.carpeta.clone();
        let s = &mut self.salas[i];
        let Some(c) = s.carga.as_mut() else {
            return Err("sec_entrega_sin_archivo".to_owned());
        };
        let sellados = c.empaquetador.empuja(datos).map_err(|e| e.to_string())?;
        Self::escribe_trozos(&mut c.archivo, &mut c.escritos, sellados)?;
        if !ultimo {
            return Ok(());
        }
        let Some(Carga {
            empaquetador,
            clave,
            mut archivo,
            nombre,
            mut escritos,
        }) = s.carga.take()
        else {
            return Ok(());
        };
        let (ultimo, huella) = empaquetador.termina().map_err(|e| e.to_string())?;
        Self::escribe_trozos(&mut archivo, &mut escritos, vec![ultimo])?;
        archivo
            .sync_all()
            .map_err(|_| "sec_err_archivo".to_owned())?;
        let n = s.e.archivos.len();
        let hojas: Vec<u8> = huella.hojas.iter().flatten().copied().collect();
        std::fs::write(carpeta.join(&s.e.id).join(format!("f{n}.hojas")), hojas)
            .map_err(|_| "sec_err_archivo".to_owned())?;
        s.e.archivos.push(Ficha {
            nombre,
            bytes: huella.bytes,
            trozos: huella.trozos,
            sha256: huella.sha256,
            raiz: huella.raiz,
            recibido: None,
        });
        s.hojas.push(Arc::new(huella.hojas));
        s.claves.push(clave);
        self.guarda(i);
        self.cambio = true;
        Ok(())
    }

    /// Every file is in: make the link and start waiting for the receiver. Returns the id.
    pub fn lista(&mut self) -> Result<String, String> {
        let i = self.preparando()?;
        if self.salas[i].carga.is_some() || self.salas[i].e.archivos.is_empty() {
            return Err("sec_entrega_vacia".to_owned());
        }
        let mut k = [0u8; 32];
        ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut k)
            .map_err(|_| "sec_err_azar".to_owned())?;
        let s = &mut self.salas[i];
        s.enlace = Some(k);
        s.e.lista();
        let id = s.e.id.clone();
        let (n, bytes) = (s.e.archivos.len(), s.e.bytes());
        self.guarda(i);
        self.anota(format!(
            "entrega {id} lista: {n} archivos, {bytes} bytes, caduca {}",
            self.salas[i].e.caducidad.nombre()
        ));
        self.arranca(i);
        self.cambio = true;
        Ok(id)
    }

    fn arranca(&mut self, i: usize) {
        let s = &mut self.salas[i];
        let Some(k) = s.enlace else { return };
        let datos = Arc::new(Datos {
            carpeta: self.carpeta.join(&s.e.id),
            hojas: s.hojas.clone(),
        });
        let (tx, rx_hilo) = tmpsc::unbounded_channel::<AlHilo>();
        let (tx_hilo, rx) = mpsc::channel::<DelHilo>();
        let rele = self.rele.clone();
        let lanzado = std::thread::Builder::new()
            .name("guardiana-entrega".to_owned())
            .spawn(move || {
                let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                else {
                    return;
                };
                rt.block_on(bucle(rele, k, datos, tx_hilo, rx_hilo));
            });
        if lanzado.is_ok() {
            s.hilo = Some(Hilo { tx, rx });
        }
    }

    /// The sender withdraws a delivery: the link stops working at once.
    pub fn revoca(&mut self, id: &str) {
        let Some(i) = self.indice(id) else { return };
        if !self.salas[i].e.estado.viva() {
            return;
        }
        self.salas[i].e.revoca();
        self.salas[i].borra_contenido(&self.carpeta);
        self.guarda(i);
        self.anota(format!("entrega {id} retirada por quien la envió"));
        self.cambio = true;
    }

    /// Destroy a delivery for an anomaly (wrong code too many times, a second browser, frames
    /// tampered with): keys wiped, sealed files deleted, the receiver told, the chain written.
    pub fn destruye(&mut self, id: &str, motivo: &str) {
        let Some(i) = self.indice(id) else { return };
        self.salas[i].e.destruye(motivo);
        self.salas[i].borra_contenido(&self.carpeta);
        self.guarda(i);
        self.anota(format!("entrega {id} destruida: {motivo}"));
        self.avisos
            .push_back(json!({ "clave": "sec_entrega_destruida", "texto": motivo }));
        self.cambio = true;
    }

    fn borra(&mut self, i: usize) {
        let s = self.salas.remove(i);
        let id = s.e.id.clone();
        drop(s);
        let _ = std::fs::remove_dir_all(self.carpeta.join(&id));
    }

    /// The person deletes a delivery from the list (its receipt goes with it from here; the
    /// chain keeps that it existed).
    pub fn olvida(&mut self, id: &str) {
        if let Some(i) = self.indice(id) {
            let mut s = self.salas.remove(i);
            s.borra_contenido(&self.carpeta);
            drop(s);
            let _ = std::fs::remove_dir_all(self.carpeta.join(id));
            self.anota(format!("entrega {id} borrada de la lista"));
            self.cambio = true;
        }
    }

    /// The signed receipt of a delivery, as JSON for a file.
    #[must_use]
    pub fn recibo(&self, id: &str) -> Option<String> {
        let s = &self.salas[self.indice(id)?];
        s.recibo
            .as_ref()
            .and_then(|r| serde_json::to_string(r).ok())
    }

    fn firma_recibo(&mut self, i: usize, receptor: Option<(&str, &str)>) {
        let contenido = self.salas[i].e.recibo(receptor);
        let Some(k) = self.clave_recibos.clone() else {
            return;
        };
        let Ok(r) = crate::recibo::firma(&k, contenido) else {
            return;
        };
        let huella = hex(&Sha256::digest(r.contenido.as_bytes()));
        let json = serde_json::to_string(&r).unwrap_or_default();
        let id = self.salas[i].e.id.clone();
        if let Some(h) = &self.salas[i].hilo {
            let _ = h.tx.send(AlHilo::ReciboFinal(json));
            let _ = h.tx.send(AlHilo::Fin("entregada".to_owned()));
        }
        self.salas[i].recibo = Some(r);
        self.salas[i].acuse = None;
        // The link is spent: the keys and the sealed files go (the receiver has them).
        self.salas[i].borra_contenido(&self.carpeta);
        self.guarda(i);
        self.anota(format!(
            "entrega {id} entregada; recibo firmado{} (SHA-256 {huella})",
            if receptor.is_some() {
                " por las dos partes"
            } else {
                " por este equipo (el receptor no firmó)"
            }
        ));
        self.avisos
            .push_back(json!({ "clave": "sec_entrega_entregada", "texto": id }));
    }

    /// Every tick: what the receivers' threads said, and the clocks.
    pub fn tic(&mut self) {
        let ahora = ahora_s();
        for i in 0..self.salas.len() {
            let eventos: Vec<DelHilo> = self.salas[i]
                .hilo
                .as_ref()
                .map(|h| h.rx.try_iter().collect())
                .unwrap_or_default();
            for ev in eventos {
                self.evento(i, ev, ahora);
            }
            if self.salas[i].e.tic(ahora) {
                let id = self.salas[i].e.id.clone();
                self.salas[i].borra_contenido(&self.carpeta);
                self.guarda(i);
                self.anota(format!("entrega {id} caducada"));
                self.cambio = true;
            }
            if let Some((_, desde)) = &self.salas[i].acuse {
                if ahora.saturating_sub(*desde) >= ACUSE_S {
                    self.firma_recibo(i, None);
                    self.cambio = true;
                }
            }
        }
    }

    fn evento(&mut self, i: usize, ev: DelHilo, ahora: u64) {
        let id = self.salas[i].e.id.clone();
        match ev {
            DelHilo::Conectado => {
                self.salas[i].conectado = true;
            }
            DelHilo::Soltado => {
                self.salas[i].conectado = false;
            }
            DelHilo::Enviado(n) => {
                self.salas[i].enviados += n;
            }
            DelHilo::Verificados(ok, mal) => {
                self.salas[i].verificados = ok;
                self.salas[i].fallidos = mal;
            }
            DelHilo::Codigo(codigo, token) => {
                let r = self.salas[i].e.intenta(&codigo, &token, ahora);
                let Ok((r, token)) = r else { return };
                match r {
                    Intento::Bien => {
                        let primera = token.is_some() && self.salas[i].e.abierta == Some(ahora);
                        let m = self.salas[i].manifiesto(&token.unwrap_or_default());
                        if let Some(h) = &self.salas[i].hilo {
                            let _ = h.tx.send(AlHilo::Bien(m));
                        }
                        self.guarda(i);
                        if primera {
                            self.anota(format!("entrega {id} abierta con el código correcto"));
                        }
                    }
                    Intento::Mal(q) => {
                        if let Some(h) = &self.salas[i].hilo {
                            let _ = h.tx.send(AlHilo::Mal(q));
                        }
                        self.guarda(i);
                        self.anota(format!("entrega {id}: código equivocado, quedan {q}"));
                    }
                    Intento::Agotada => self.destruye(&id, "codigo_distinto"),
                    Intento::OtroDispositivo => self.destruye(&id, "dispositivo_nuevo"),
                    Intento::NoVigente => {
                        if let Some(h) = &self.salas[i].hilo {
                            let _ =
                                h.tx.send(AlHilo::Fin(self.salas[i].e.estado.nombre().to_owned()));
                        }
                    }
                }
            }
            DelHilo::Recibido(n, raiz) => {
                if self.salas[i].e.recibido(n, &raiz, ahora) {
                    self.guarda(i);
                    if self.salas[i].e.estado == Estado::Entregada {
                        let contenido = self.salas[i].e.recibo(None);
                        if let Some(h) = &self.salas[i].hilo {
                            let _ = h.tx.send(AlHilo::Recibo(contenido.clone()));
                        }
                        self.salas[i].acuse = Some((contenido, ahora));
                    }
                }
            }
            DelHilo::Acuse(clave, firma) => {
                let Some((contenido, _)) = self.salas[i].acuse.clone() else {
                    return;
                };
                let ok = match (B64.decode(&clave), B64.decode(&firma)) {
                    (Ok(k), Ok(f)) => ring::signature::UnparsedPublicKey::new(
                        &ring::signature::ECDSA_P256_SHA256_FIXED,
                        k,
                    )
                    .verify(contenido.as_bytes(), &f)
                    .is_ok(),
                    _ => false,
                };
                if ok {
                    self.firma_recibo(i, Some((&clave, &firma)));
                } else {
                    // A signature that does not check is no receipt; this computer signs alone.
                    self.firma_recibo(i, None);
                }
            }
            DelHilo::Manipulado => self.destruye(&id, "manipulada"),
        }
        self.cambio = true;
    }

    /// Notices for the page (drained).
    pub fn avisos(&mut self) -> Vec<Value> {
        self.avisos.drain(..).collect()
    }

    /// Whether the screen must be redrawn, and clear the flag.
    pub fn cambiado(&mut self) -> bool {
        std::mem::take(&mut self.cambio)
    }

    /// The fingerprint of the key that signs the receipts.
    #[must_use]
    pub fn huella(&self) -> String {
        self.clave_recibos
            .as_ref()
            .and_then(|k| crate::recibo::huella(k).ok())
            .unwrap_or_default()
    }

    /// What the page draws: every delivery, with its link and code while it is open.
    #[must_use]
    pub fn json(&self, idioma: &str) -> Value {
        let lista: Vec<Value> = self
            .salas
            .iter()
            .rev()
            .map(|s| {
                let mut v = s.e.json();
                let enlace = s.enlace_texto(&self.rele, idioma);
                v["qr"] = json!(enlace
                    .as_deref()
                    .and_then(|l| guardiana_secure::tarjeta::qr_svg(l).ok()));
                v["enlace"] = json!(enlace);
                v["conectado"] = json!(s.conectado);
                v["recibo"] = json!(s.recibo.is_some());
                v["subiendo"] = json!(s
                    .carga
                    .as_ref()
                    .map(|c| json!({ "nombre": c.nombre, "trozos": c.escritos })));
                v["progreso"] = json!({
                    "trozos_enviados": s.enviados,
                    "trozos_total": s.e.archivos.iter().map(|a| a.trozos).sum::<u64>(),
                    "verificados": s.verificados,
                    "fallidos": s.fallidos,
                });
                v
            })
            .collect();
        json!({ "tipo": "entregas", "entregas": lista, "huella": self.huella(), "dpapi": crate::dpapi::disponible() })
    }

    /// The deliveries' chain as the page shows it (unsigned; [`crate::grupo::Grupo::firma_cadena`]
    /// signs it for a file).
    #[must_use]
    pub fn cadena(&self) -> Cadena {
        self.cadena.clone()
    }
}

/// Wait at the relay for the receiver, over and over, until this delivery ends.
async fn bucle(
    rele: String,
    clave: [u8; 32],
    datos: Arc<Datos>,
    tx: Sender<DelHilo>,
    mut rx: tmpsc::UnboundedReceiver<AlHilo>,
) {
    let sala = sala_de(&clave);
    loop {
        let desde = std::time::Instant::now();
        if una_vez(&rele, &sala, &clave, &datos, &tx, &mut rx)
            .await
            .unwrap_or(false)
        {
            return;
        }
        let _ = tx.send(DelHilo::Soltado);
        let pausa = if desde.elapsed() > Duration::from_secs(30) {
            1
        } else {
            3
        };
        tokio::select! {
            () = tokio::time::sleep(Duration::from_secs(pausa)) => {}
            m = rx.recv() => {
                if matches!(m, None | Some(AlHilo::Fin(_))) {
                    return;
                }
            }
        }
    }
}

/// One piece as it travels: file, number, whether it is the last, the sealed bytes.
fn trama_trozo(i: u32, n: u32, ultimo: bool, sellado: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(9 + sellado.len());
    v.extend_from_slice(&i.to_be_bytes());
    v.extend_from_slice(&n.to_be_bytes());
    v.push(u8::from(ultimo));
    v.extend_from_slice(sellado);
    v
}

fn lee_trozo(carpeta: &Path, i: usize, n: u64) -> Option<Vec<u8>> {
    let mut f = File::open(carpeta.join(format!("f{i}.bin"))).ok()?;
    f.seek(SeekFrom::Start(n.checked_mul(PASO)?)).ok()?;
    let mut l = [0u8; 4];
    f.read_exact(&mut l).ok()?;
    let l = u32::from_be_bytes(l) as usize;
    if l > TROZO + 16 {
        return None;
    }
    let mut v = vec![0u8; l];
    f.read_exact(&mut v).ok()?;
    Some(v)
}

/// A JSON frame for the page, padded to its size bucket (spaces after the JSON).
fn json_para(v: &Value) -> (u8, Vec<u8>) {
    let mut b = v.to_string().into_bytes();
    relleno::rellena(&mut b, b' ', 64 * 1024);
    (0, b)
}

/// One stay at the relay: wait for the receiver, greet it, serve it until it goes. `Some(true)`
/// when the delivery ended and nothing more is to be waited for.
async fn una_vez(
    rele: &str,
    sala: &str,
    clave: &[u8; 32],
    datos: &Datos,
    tx: &Sender<DelHilo>,
    rx: &mut tmpsc::UnboundedReceiver<AlHilo>,
) -> Option<bool> {
    let mut s = movil::conecta_rele(rele).await?;
    let _ = s.set_nodelay(true);
    s.write_all(format!("GUARDIANA-RELE-M 1 {sala}\n").as_bytes())
        .await
        .ok()?;
    let mut ok = [0u8; 3];
    tokio::select! {
        r = tokio::time::timeout(Duration::from_secs(11 * 60), s.read_exact(&mut ok)) => {
            r.ok()?.ok()?;
        }
        m = rx.recv() => {
            if matches!(m, None | Some(AlHilo::Fin(_))) {
                return Some(true);
            }
            return Some(false);
        }
    }
    if &ok != b"OK\n" {
        return None;
    }
    // The phone's greeting («GM2», `movil`): our number and a fresh P-256 key; the page answers
    // with its own and proves it holds the link key.
    let rng = ring::rand::SystemRandom::new();
    let privada =
        ring::agreement::EphemeralPrivateKey::generate(&ring::agreement::ECDH_P256, &rng).ok()?;
    let publica = privada.compute_public_key().ok()?;
    let mut del_equipo = [0u8; 16];
    crate::sesion::azar(&mut del_equipo);
    let mut saludo = b"GM2".to_vec();
    saludo.extend_from_slice(&del_equipo);
    saludo.extend_from_slice(publica.as_ref());
    movil::escribe_rele(&mut s, &saludo).await?;
    let respuesta = tokio::time::timeout(Duration::from_secs(20), movil::lee_rele(&mut s))
        .await
        .ok()??;
    if respuesta.len() < 16 + 65 {
        return None;
    }
    let mut del_movil = [0u8; 16];
    del_movil.copy_from_slice(&respuesta[..16]);
    let suya = ring::agreement::UnparsedPublicKey::new(
        &ring::agreement::ECDH_P256,
        respuesta[16..16 + 65].to_vec(),
    );
    let secreto = ring::agreement::agree_ephemeral(privada, &suya, |s| s.to_vec()).ok()?;
    let (k_me, k_em) = movil::claves_de_enlace(clave, &secreto, &del_equipo, &del_movil)?;
    if movil::abre(&k_me, 0, b"GM2", &respuesta[16 + 65..]).as_deref() != Some(b"hola".as_slice()) {
        return None;
    }
    let _ = tx.send(DelHilo::Conectado);
    let (mut r, mut w) = s.into_split();
    // Frames to the page, sealed with the next number; an empty one every 25 s so no network
    // in between drops a quiet link.
    let (tx_sale, mut rx_sale) = tmpsc::unbounded_channel::<(u8, Vec<u8>)>();
    let escritor = tokio::spawn(async move {
        let mut n: u64 = 0;
        let mut latido = tokio::time::interval(Duration::from_secs(25));
        latido.tick().await;
        loop {
            let (tipo, cuerpo) = tokio::select! {
                f = rx_sale.recv() => match f { Some(f) => f, None => break },
                _ = latido.tick() => (3u8, Vec::new()),
            };
            let mut plano = Vec::with_capacity(1 + cuerpo.len());
            plano.push(tipo);
            plano.extend_from_slice(&cuerpo);
            let sellado = movil::sella(&k_em, n, b"", &plano);
            n += 1;
            if movil::escribe_rele(&mut w, &sellado).await.is_none() {
                break;
            }
        }
        let _ = w.shutdown().await;
    });
    // Frames from the page: frame 0 was the proof, so they count from 1. A frame that does not
    // open is a tampering (a cut link just ends the reading).
    let (tx_entra, mut rx_entra) = tmpsc::unbounded_channel::<Option<Vec<u8>>>();
    let lector = tokio::spawn(async move {
        let mut n: u64 = 1;
        while let Some(sellado) = movil::lee_rele(&mut r).await {
            let Some(plano) = movil::abre(&k_me, n, b"", &sellado) else {
                let _ = tx_entra.send(None);
                return;
            };
            n += 1;
            if tx_entra.send(Some(plano)).is_err() {
                return;
            }
        }
    });
    let _ = tx_sale.send(json_para(&json!({ "t": "hola", "v": 1 })));
    let mut autorizado = false;
    let mut terminar = false;
    loop {
        tokio::select! {
            f = rx_entra.recv() => {
                let Some(f) = f else { break };
                let Some(plano) = f else {
                    let _ = tx.send(DelHilo::Manipulado);
                    break;
                };
                let Some((&tipo, cuerpo)) = plano.split_first() else { continue };
                if tipo != 0 {
                    continue;
                }
                let Ok(v) = serde_json::from_slice::<Value>(cuerpo) else { continue };
                match v["t"].as_str().unwrap_or_default() {
                    "codigo" => {
                        let c: String = v["codigo"].as_str().unwrap_or_default().chars().take(16).collect();
                        let t: String = v["token"].as_str().unwrap_or_default().chars().take(64).collect();
                        let _ = tx.send(DelHilo::Codigo(c, t));
                    }
                    "pide" if autorizado => {
                        let _ = tx.send(DelHilo::Verificados(
                            v["ok"].as_u64().unwrap_or(0),
                            v["mal"].as_u64().unwrap_or(0),
                        ));
                        let i = v["i"].as_u64().unwrap_or(u64::MAX) as usize;
                        let Some(hojas) = datos.hojas.get(i) else { continue };
                        let total = hojas.len() as u64;
                        let desde = v["desde"].as_u64().unwrap_or(0).min(total);
                        // A resume: the root of what the page holds must be ours.
                        if desde > 0 {
                            let suya = v["raiz_previa"].as_str().unwrap_or_default();
                            let nuestra = arbol::hex(&arbol::raiz(&hojas[..desde as usize]));
                            if suya != nuestra {
                                let _ = tx_sale.send(json_para(&json!({ "t": "reinicia", "i": i })));
                                continue;
                            }
                        }
                        let cuantos = v["cuantos"].as_u64().unwrap_or(1).clamp(1, PIDE_MAX);
                        let mut mandados = 0;
                        for n in desde..(desde + cuantos).min(total) {
                            let Some(t) = lee_trozo(&datos.carpeta, i, n) else {
                                let _ = tx_sale.send(json_para(&json!({ "t": "fin", "motivo": "sin_archivo" })));
                                break;
                            };
                            let _ = tx_sale.send((2, trama_trozo(i as u32, n as u32, n + 1 == total, &t)));
                            mandados += 1;
                        }
                        let _ = tx.send(DelHilo::Enviado(mandados));
                    }
                    "recibido" if autorizado => {
                        let i = v["i"].as_u64().unwrap_or(u64::MAX) as usize;
                        let raiz: String = v["raiz"].as_str().unwrap_or_default().chars().take(64).collect();
                        let _ = tx.send(DelHilo::Recibido(i, raiz));
                    }
                    "acuse" if autorizado => {
                        let c: String = v["clave"].as_str().unwrap_or_default().chars().take(200).collect();
                        let f: String = v["firma"].as_str().unwrap_or_default().chars().take(200).collect();
                        let _ = tx.send(DelHilo::Acuse(c, f));
                    }
                    _ => {}
                }
            }
            m = rx.recv() => {
                match m {
                    Some(AlHilo::Bien(manifiesto)) => {
                        autorizado = true;
                        let mut b = manifiesto.into_bytes();
                        relleno::rellena(&mut b, b' ', 1024 * 1024);
                        let _ = tx_sale.send((0, b));
                    }
                    Some(AlHilo::Mal(q)) => {
                        let _ = tx_sale.send(json_para(&json!({ "t": "codigo_mal", "quedan": q })));
                    }
                    Some(AlHilo::Recibo(c)) => {
                        let _ = tx_sale.send(json_para(&json!({ "t": "recibo", "contenido": c })));
                    }
                    Some(AlHilo::ReciboFinal(j)) => {
                        let _ = tx_sale.send(json_para(&json!({ "t": "recibo_final", "recibo": j })));
                    }
                    Some(AlHilo::Fin(motivo)) => {
                        let _ = tx_sale.send(json_para(&json!({ "t": "fin", "motivo": motivo })));
                        terminar = true;
                        break;
                    }
                    None => {
                        terminar = true;
                        break;
                    }
                }
            }
        }
    }
    // Let the last frames leave before the link goes.
    drop(tx_sale);
    let _ = tokio::time::timeout(Duration::from_secs(3), escritor).await;
    lector.abort();
    Some(terminar)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn carpeta(nombre: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("guardiana-entrega-{}-{nombre}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let _ = std::fs::create_dir_all(&d);
        d
    }

    fn llena(e: &mut Entregas, archivos: &[(&str, Vec<u8>)]) -> String {
        e.nueva("24h", 6, true, "prueba").unwrap();
        for (n, datos) in archivos {
            e.archivo(n, datos.len() as u64).unwrap();
            let partes: Vec<&[u8]> = datos.chunks(65536).collect();
            if partes.is_empty() {
                e.trozo(&[], true).unwrap();
            }
            for (k, p) in partes.iter().enumerate() {
                e.trozo(p, k + 1 == partes.len()).unwrap();
            }
        }
        // No relay in these tests: `lista` would start waiting at one.
        e.salas.last().unwrap().e.id.clone()
    }

    #[test]
    fn files_go_in_sealed_and_the_state_survives_a_restart_without_the_keys_in_the_clear() {
        let d = carpeta("guarda");
        let grande: Vec<u8> = (0..(TROZO + 5000)).map(|i| (i % 253) as u8).collect();
        let id = {
            let mut e = Entregas::abre_con(&d, "127.0.0.1:9");
            let id = llena(
                &mut e,
                &[
                    ("../../informes\\marzo.pdf", grande.clone()),
                    ("vacío.txt", Vec::new()),
                ],
            );
            let s = &e.salas[0];
            assert_eq!(s.e.archivos.len(), 2);
            assert_eq!(
                s.e.archivos[0].nombre, "informes/marzo.pdf",
                "no way out of the folder"
            );
            assert_eq!(s.e.archivos[0].trozos, 2);
            assert_eq!(s.e.archivos[1].trozos, 1);
            // What is on the disk is ciphertext: no run of the file's bytes is there.
            let f0 = std::fs::read(d.join("secure/entregas").join(&id).join("f0.bin")).unwrap();
            assert_eq!(f0.len() as u64, 2 * PASO);
            assert!(!f0.windows(64).any(|w| w == &grande[1000..1064]));
            let estado =
                std::fs::read(d.join("secure/entregas").join(&id).join("estado.bin")).unwrap();
            assert!(!String::from_utf8_lossy(&estado).contains(s.e.codigo()));
            // A stored piece opens with the file's key, and is the first MiB.
            let t = lee_trozo(&d.join("secure/entregas").join(&id), 0, 0).unwrap();
            let f = flujo::Flujo::new(&s.claves[0], flujo::contexto(&id, 0)).unwrap();
            assert_eq!(f.abre(0, false, &t).unwrap(), &grande[..TROZO]);
            id
        };
        // Back after a restart: the same delivery, its keys, its leaves.
        let e = Entregas::abre_con(&d, "127.0.0.1:9");
        let s = &e.salas[0];
        assert_eq!(s.e.id, id);
        assert_eq!(s.claves.len(), 2);
        assert_eq!(s.hojas[0].len(), 2);
        assert_eq!(arbol::hex(&arbol::raiz(&s.hojas[0])), s.e.archivos[0].raiz);
        // It was still being filled when the program closed: withdrawn.
        assert_eq!(s.e.estado, Estado::Revocada);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn an_expired_or_destroyed_delivery_loses_its_keys_and_files_and_the_chain_says_so() {
        let d = carpeta("caduca");
        let mut e = Entregas::abre_con(&d, "127.0.0.1:9");
        let id = llena(&mut e, &[("a.bin", vec![1u8; 10])]);
        e.salas[0].e.lista();
        e.salas[0].e.vence = 0;
        e.tic();
        assert_eq!(e.salas[0].e.estado, Estado::Caducada);
        assert!(e.salas[0].claves.is_empty());
        assert!(!d.join("secure/entregas").join(&id).join("f0.bin").exists());
        assert!(e.json("es")["entregas"][0]["enlace"].is_null());
        let id2 = llena(&mut e, &[("b.bin", vec![2u8; 10])]);
        e.salas[1].e.lista();
        e.destruye(&id2, "manipulada");
        assert_eq!(e.salas[1].e.estado, Estado::Destruida("manipulada".into()));
        assert!(!d.join("secure/entregas").join(&id2).join("f0.bin").exists());
        let textos: Vec<&str> = e
            .cadena
            .registros
            .iter()
            .map(|r| r.texto.as_str())
            .collect();
        assert!(textos.iter().any(|t| t.contains("caducada")), "{textos:?}");
        assert!(
            textos.iter().any(|t| t.contains("destruida: manipulada")),
            "{textos:?}"
        );
        assert!(e.cadena.comprueba().is_ok());
        let avisos = e.avisos();
        assert!(avisos.iter().any(|a| a["clave"] == "sec_entrega_destruida"));
        let _ = std::fs::remove_dir_all(&d);
    }

    async fn escribe_ws(t: &mut tokio::net::TcpStream, datos: &[u8]) {
        let m = [9u8, 8, 7, 6];
        let mut c = vec![0x82u8];
        if datos.len() < 126 {
            c.push(0x80 | datos.len() as u8);
        } else {
            c.push(0x80 | 126);
            c.extend_from_slice(&(datos.len() as u16).to_be_bytes());
        }
        c.extend_from_slice(&m);
        c.extend(datos.iter().enumerate().map(|(i, b)| b ^ m[i % 4]));
        t.write_all(&c).await.unwrap();
    }

    async fn lee_ws(t: &mut tokio::net::TcpStream) -> Vec<u8> {
        loop {
            let mut h = [0u8; 2];
            t.read_exact(&mut h).await.unwrap();
            let n = match h[1] & 0x7f {
                126 => {
                    let mut b = [0u8; 2];
                    t.read_exact(&mut b).await.unwrap();
                    u16::from_be_bytes(b) as usize
                }
                127 => {
                    let mut b = [0u8; 8];
                    t.read_exact(&mut b).await.unwrap();
                    u64::from_be_bytes(b) as usize
                }
                n => n as usize,
            };
            let mut v = vec![0u8; n];
            t.read_exact(&mut v).await.unwrap();
            if h[0] & 0x0f == 0x2 {
                return v;
            }
        }
    }

    /// A browser that read the link: through the relay, the greeting, then sealed frames.
    struct Navegador {
        t: tokio::net::TcpStream,
        k_me: aead::LessSafeKey,
        k_em: aead::LessSafeKey,
        n_sale: u64,
        n_entra: u64,
    }

    impl Navegador {
        async fn entra(dir: &str, enlace: &str) -> Self {
            let datos = enlace
                .strip_prefix(PAGINA)
                .unwrap()
                .strip_prefix('#')
                .unwrap();
            let d: Value = serde_json::from_slice(&B64URL.decode(datos).unwrap()).unwrap();
            let clave: [u8; 32] = B64URL
                .decode(d["k"].as_str().unwrap())
                .unwrap()
                .try_into()
                .unwrap();
            let sala = sala_de(&clave);
            let mut t = tokio::net::TcpStream::connect(dir).await.unwrap();
            t.write_all(format!("GET /m/{sala} HTTP/1.1\r\nHost: rele\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\nOrigin: https://guardianagroup.com\r\n\r\n").as_bytes()).await.unwrap();
            let mut r = Vec::new();
            while !r.ends_with(b"\r\n\r\n") {
                let mut b = [0u8; 1];
                t.read_exact(&mut b).await.unwrap();
                r.push(b[0]);
            }
            assert!(r.starts_with(b"HTTP/1.1 101"));
            let saludo = lee_ws(&mut t).await;
            assert_eq!(&saludo[..3], b"GM2");
            let del_equipo: [u8; 16] = saludo[3..19].try_into().unwrap();
            let rng = ring::rand::SystemRandom::new();
            let privada =
                ring::agreement::EphemeralPrivateKey::generate(&ring::agreement::ECDH_P256, &rng)
                    .unwrap();
            let publica = privada.compute_public_key().unwrap();
            let suya = ring::agreement::UnparsedPublicKey::new(
                &ring::agreement::ECDH_P256,
                saludo[19..].to_vec(),
            );
            let secreto = ring::agreement::agree_ephemeral(privada, &suya, |s| s.to_vec()).unwrap();
            let del_movil = [5u8; 16];
            let (k_me, k_em) =
                movil::claves_de_enlace(&clave, &secreto, &del_equipo, &del_movil).unwrap();
            let mut primera = del_movil.to_vec();
            primera.extend_from_slice(publica.as_ref());
            primera.extend(movil::sella(&k_me, 0, b"GM2", b"hola"));
            escribe_ws(&mut t, &primera).await;
            Self {
                t,
                k_me,
                k_em,
                n_sale: 1,
                n_entra: 0,
            }
        }

        async fn manda(&mut self, v: Value) {
            let mut plano = vec![0u8];
            plano.extend_from_slice(v.to_string().as_bytes());
            let s = movil::sella(&self.k_me, self.n_sale, b"", &plano);
            self.n_sale += 1;
            escribe_ws(&mut self.t, &s).await;
        }

        /// The next frame that is not a heartbeat: (type, body).
        async fn lee(&mut self) -> (u8, Vec<u8>) {
            loop {
                let v = tokio::time::timeout(Duration::from_secs(10), lee_ws(&mut self.t))
                    .await
                    .unwrap();
                let p = movil::abre(&self.k_em, self.n_entra, b"", &v).unwrap();
                self.n_entra += 1;
                if p[0] != 3 {
                    return (p[0], p[1..].to_vec());
                }
            }
        }

        async fn lee_json(&mut self) -> Value {
            loop {
                let (t, b) = self.lee().await;
                if t == 0 {
                    return serde_json::from_slice(&b).unwrap();
                }
            }
        }
    }

    /// Wait until the engine sees what the browser did.
    fn espera(e: &mut Entregas, cond: impl Fn(&Entregas) -> bool) {
        for _ in 0..200 {
            e.tic();
            if cond(e) {
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        e.tic();
        assert!(cond(e), "the engine did not get there");
    }

    #[test]
    fn a_receiver_with_the_code_gets_every_piece_and_both_sides_sign_the_receipt() {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let listener = rt
            .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
            .unwrap();
        let dir = listener.local_addr().unwrap().to_string();
        rt.spawn(guardiana_rele::sirve(listener));
        let d = carpeta("extremo");
        let mut e = Entregas::abre_con(&d, &dir);
        let grande: Vec<u8> = (0..(2 * TROZO + 777)).map(|i| (i % 241) as u8).collect();
        let id = llena(
            &mut e,
            &[
                ("contrato.pdf", grande.clone()),
                ("nota.txt", b"hola".to_vec()),
            ],
        );
        e.lista().unwrap();
        let codigo = e.salas[0].e.codigo().to_owned();
        let enlace = e.json("es")["entregas"][0]["enlace"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(enlace.starts_with(PAGINA));
        assert!(
            !enlace.contains(&codigo),
            "the code never travels in the link"
        );
        std::thread::sleep(Duration::from_millis(300));
        let mut nav = rt.block_on(Navegador::entra(&dir, &enlace));
        assert_eq!(rt.block_on(nav.lee_json())["t"], "hola");
        // A wrong code first: told how many tries are left, nothing else.
        let malo = if codigo == "000000" {
            "111111"
        } else {
            "000000"
        };
        rt.block_on(nav.manda(json!({ "t": "codigo", "codigo": malo, "token": "" })));
        espera(&mut e, |e| e.salas[0].e.fallos == 1);
        let r = rt.block_on(nav.lee_json());
        assert_eq!(
            (r["t"].as_str(), r["quedan"].as_u64()),
            (Some("codigo_mal"), Some(2))
        );
        // Pieces asked before the code are not served.
        rt.block_on(nav.manda(json!({ "t": "pide", "i": 0, "desde": 0, "cuantos": 2 })));
        rt.block_on(nav.manda(json!({ "t": "codigo", "codigo": codigo, "token": "" })));
        espera(&mut e, |e| e.salas[0].e.estado == Estado::Abierta);
        let m = rt.block_on(nav.lee_json());
        assert_eq!(m["t"], "manifiesto");
        assert_eq!(m["archivos"].as_array().unwrap().len(), 2);
        // Every file, piece by piece, opened with its own key and checked against its root.
        for a in m["archivos"].as_array().unwrap() {
            let i = a["i"].as_u64().unwrap();
            let total = a["trozos"].as_u64().unwrap();
            let k: [u8; 32] = B64
                .decode(a["clave"].as_str().unwrap())
                .unwrap()
                .try_into()
                .unwrap();
            let p: [u8; 7] = B64
                .decode(a["prefijo"].as_str().unwrap())
                .unwrap()
                .try_into()
                .unwrap();
            let f = flujo::Flujo::new(&Clave::de(k, p), flujo::contexto(&id, i as u32)).unwrap();
            let mut plano = Vec::new();
            let mut hojas = Vec::new();
            let mut n = 0;
            while n < total {
                let mut pide =
                    json!({ "t": "pide", "i": i, "desde": n, "cuantos": 1, "ok": n, "mal": 0 });
                if n > 0 {
                    pide["raiz_previa"] = json!(arbol::hex(&arbol::raiz(&hojas)));
                }
                rt.block_on(nav.manda(pide));
                let (tipo, b) = rt.block_on(nav.lee());
                assert_eq!(tipo, 2);
                let fi = u32::from_be_bytes(b[0..4].try_into().unwrap());
                let fnum = u32::from_be_bytes(b[4..8].try_into().unwrap());
                assert_eq!((u64::from(fi), u64::from(fnum)), (i, n));
                let t = f.abre(fnum, b[8] == 1, &b[9..]).unwrap();
                hojas.push(arbol::hoja(&t));
                plano.extend(t);
                n += 1;
            }
            if i == 0 {
                assert_eq!(plano, grande);
                // A resume against another root: that file starts again.
                rt.block_on(nav.manda(
                    json!({ "t": "pide", "i": 0, "desde": 1, "cuantos": 1, "raiz_previa": "00" }),
                ));
                let v = rt.block_on(nav.lee_json());
                assert_eq!(
                    (v["t"].as_str(), v["i"].as_u64()),
                    (Some("reinicia"), Some(0))
                );
            } else {
                assert_eq!(plano, b"hola");
            }
            let raiz = arbol::hex(&arbol::raiz(&hojas));
            assert_eq!(raiz, a["raiz"].as_str().unwrap());
            rt.block_on(nav.manda(json!({ "t": "recibido", "i": i, "raiz": raiz })));
        }
        espera(&mut e, |e| e.salas[0].acuse.is_some());
        let recibo = rt.block_on(nav.lee_json());
        assert_eq!(recibo["t"], "recibo");
        let contenido = recibo["contenido"].as_str().unwrap().to_owned();
        let c: Value = serde_json::from_str(&contenido).unwrap();
        assert_eq!(c["estado"], "entregado");
        assert_eq!(c["archivos"][0]["sha256"], hex(&Sha256::digest(&grande)));
        // The browser signs it with a key of its own (as WebCrypto would: P-256, r‖s).
        let rng = ring::rand::SystemRandom::new();
        let pk8 = ring::signature::EcdsaKeyPair::generate_pkcs8(
            &ring::signature::ECDSA_P256_SHA256_FIXED_SIGNING,
            &rng,
        )
        .unwrap();
        let par = ring::signature::EcdsaKeyPair::from_pkcs8(
            &ring::signature::ECDSA_P256_SHA256_FIXED_SIGNING,
            pk8.as_ref(),
            &rng,
        )
        .unwrap();
        use ring::signature::KeyPair as _;
        let firma = par.sign(&rng, contenido.as_bytes()).unwrap();
        rt.block_on(nav.manda(json!({
            "t": "acuse",
            "clave": B64.encode(par.public_key().as_ref()),
            "firma": B64.encode(firma.as_ref()),
        })));
        espera(&mut e, |e| e.salas[0].recibo.is_some());
        let final_ = rt.block_on(nav.lee_json());
        assert_eq!(final_["t"], "recibo_final");
        let r: crate::recibo::Recibo =
            serde_json::from_str(final_["recibo"].as_str().unwrap()).unwrap();
        assert_eq!(crate::recibo::comprueba(&r), Ok(()));
        let v: Value = serde_json::from_str(&r.contenido).unwrap();
        assert_eq!(v["receptor"]["firma"], B64.encode(firma.as_ref()));
        assert_eq!(rt.block_on(nav.lee_json())["t"], "fin");
        // Spent: keys and sealed files gone, the receipt kept, the chain written.
        let s = &e.salas[0];
        assert_eq!(s.e.estado, Estado::Entregada);
        assert!(s.claves.is_empty());
        assert!(e.recibo(&id).is_some());
        assert!(!d.join("secure/entregas").join(&id).join("f0.bin").exists());
        assert!(e
            .cadena
            .registros
            .iter()
            .any(|r| r.texto.contains("por las dos partes")));
        let _ = std::fs::remove_dir_all(&d);
    }
}
