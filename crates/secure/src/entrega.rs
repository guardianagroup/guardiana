//! «Sala de entrega» (18 Oct 2026, decided with the committee — Claude, ChatGPT, Grok, Kimi — and
//! the owner): a company hands files of any kind and size to someone outside, from its computer,
//! with a link that works once and a separate code that proves the right person opened it. The
//! receiver needs nothing but a browser; the relay in between only ever carries ciphertext; at
//! the end both sides keep the same signed receipt.
//!
//! This module is the delivery's rules, with no network and no disk: when it expires, how the
//! code is checked, when the link is spent, and what the receipt says. The program around it
//! (`guardiana-zero`'s `entrega`) carries the bytes and keeps the state.
//!
//! The rules, each one a sentence the screen also says:
//! - The link carries a 256-bit key in the part of the address a browser never sends (`#…`);
//!   the code (6 digits by default, 4 to 12) goes by another road (a call, a message). Whoever
//!   has only the link cannot open anything: the computer asks for the code inside the sealed
//!   channel and sends the files' keys only after it.
//! - A wrong code leaves [`INTENTOS`] − 1 more tries; the last wrong one destroys the delivery
//!   (keys wiped, files unreachable). The code is checked by the computer, online, so it cannot
//!   be guessed offline from a recorded link.
//! - The first receiver that gives the right code is the only one: it gets a token for its
//!   tab, and a second browser that comes with the link afterwards is «a new device» — the
//!   delivery is destroyed as an anomaly ([`Intento::OtroDispositivo`]).
//! - It ends at its expiry (24 hours, 7 days) or, if so chosen, at the first complete download;
//!   once everything was received the link is spent in every case.
//! - The receipt names the files only if the sender chose so; it always carries sizes, the
//!   SHA-256 of every file and the root of its hash tree, the times, «entregado» and «descargado
//!   completo». It never carries an address: the computer meets the receiver through the relay
//!   and never learns one (the owner: never store IPs).
//!
//! Strength, honestly: the files travel sealed twice — by the channel (ECDH P-256 mixed with the
//! link's key, AES-256-GCM, the same channel the phone uses) and by each file's own STREAM key
//! ([`crate::flujo`]). A quantum computer that broke P-256 would still need the link's 256-bit
//! key. ML-KEM is not used here: the receiver's browser has no ML-KEM in WebCrypto, and a
//! JavaScript copy would be unaudited code; the conversations between two GUARDIANA computers
//! keep their ML-KEM-768 layer ([`crate::cuantico`]). ML-KEM-1024 was evaluated (the `ml-kem`
//! crate offers it) and left out for the same reason: there is no second GUARDIANA on the
//! receiver's side to run it.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Wrong codes allowed in total; the last one destroys the delivery.
pub const INTENTOS: u32 = 3;
/// Shortest and longest codes.
pub const DIGITOS_MIN: usize = 4;
/// See [`DIGITOS_MIN`].
pub const DIGITOS_MAX: usize = 12;
/// The default code length.
pub const DIGITOS: usize = 6;

/// When a delivery ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Caducidad {
    /// After this many hours (24 h, 7 days = 168 h).
    Horas(u64),
    /// At the first complete download (and in 7 days at most).
    PrimeraDescarga,
}

impl Caducidad {
    /// The seconds it may live at most.
    #[must_use]
    pub fn segundos(self) -> u64 {
        match self {
            Self::Horas(h) => h.clamp(1, 24 * 30) * 3600,
            Self::PrimeraDescarga => 7 * 24 * 3600,
        }
    }

    /// The name the receipt and the page use.
    #[must_use]
    pub fn nombre(self) -> String {
        match self {
            Self::Horas(h) if h % 24 == 0 => format!("{}d", h / 24),
            Self::Horas(h) => format!("{h}h"),
            Self::PrimeraDescarga => "primera_descarga".to_owned(),
        }
    }

    /// From the page's choice (`24h`, `7d`, `primera_descarga`).
    #[must_use]
    pub fn de(s: &str) -> Self {
        match s {
            "primera_descarga" => Self::PrimeraDescarga,
            "7d" => Self::Horas(168),
            _ => Self::Horas(24),
        }
    }
}

/// Where a delivery stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "estado", content = "motivo")]
pub enum Estado {
    /// Files still going in.
    Preparando,
    /// Ready, waiting for the receiver.
    Esperando,
    /// The right code was given; files going out.
    Abierta,
    /// Everything was received: the link is spent.
    Entregada,
    /// Its time ran out.
    Caducada,
    /// The sender withdrew it.
    Revocada,
    /// Destroyed for an anomaly; the reason as a short key.
    Destruida(String),
}

impl Estado {
    /// The delivery can still be opened or finished.
    #[must_use]
    pub fn viva(&self) -> bool {
        matches!(self, Self::Preparando | Self::Esperando | Self::Abierta)
    }

    /// The short name for the page.
    #[must_use]
    pub fn nombre(&self) -> &str {
        match self {
            Self::Preparando => "preparando",
            Self::Esperando => "esperando",
            Self::Abierta => "abierta",
            Self::Entregada => "entregada",
            Self::Caducada => "caducada",
            Self::Revocada => "revocada",
            Self::Destruida(_) => "destruida",
        }
    }
}

/// One file of a delivery, as the receipt describes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ficha {
    /// The name with its folder, as the sender chose it (`informes/marzo.pdf`).
    pub nombre: String,
    /// Plaintext bytes.
    pub bytes: u64,
    /// Pieces ([`crate::flujo::TROZO`] each).
    pub trozos: u64,
    /// SHA-256 of the whole file, hex.
    pub sha256: String,
    /// Root of its hash tree ([`crate::arbol`]), hex.
    pub raiz: String,
    /// The receiver confirmed it whole, with the same root (unix s).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recibido: Option<u64>,
}

/// What a code given by the receiver leads to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intento {
    /// Right: the files' keys may go; the token is the one this receiver must keep using.
    Bien,
    /// Wrong; this many tries left.
    Mal(u32),
    /// Wrong for the last time: the delivery was destroyed.
    Agotada,
    /// Right code, but another browser had already opened it: destroyed as an anomaly.
    OtroDispositivo,
    /// The delivery is not open any more (expired, spent, withdrawn, destroyed).
    NoVigente,
}

/// One delivery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entrega {
    /// 16 hex digits, random.
    pub id: String,
    /// The sender's own label, never sent (empty: none).
    #[serde(default)]
    pub titulo: String,
    /// Unix seconds.
    pub creada: u64,
    /// The rule it ends by.
    pub caducidad: Caducidad,
    /// When it ends at the latest.
    pub vence: u64,
    codigo: String,
    /// Wrong codes so far.
    pub fallos: u32,
    /// The token of the receiver that opened it (hex), once it did.
    receptor: Option<String>,
    /// Where it stands.
    pub estado: Estado,
    /// Its files.
    pub archivos: Vec<Ficha>,
    /// When the right code was given.
    pub abierta: Option<u64>,
    /// When the last file was confirmed whole.
    pub completa: Option<u64>,
    /// The receipt names the files (the sender's choice; sizes and hashes always).
    pub nombres_en_recibo: bool,
}

fn azar_hex(n: usize) -> Result<String, crate::Error> {
    let mut b = vec![0u8; n];
    getrandom::fill(&mut b).map_err(|_| crate::Error::CannotStart)?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

/// A code of `digitos` decimal digits, each one uniform (bytes ≥ 250 are drawn again, so no
/// digit is likelier than another).
pub fn codigo_nuevo(digitos: usize) -> Result<String, crate::Error> {
    let digitos = digitos.clamp(DIGITOS_MIN, DIGITOS_MAX);
    let mut s = String::with_capacity(digitos);
    while s.len() < digitos {
        let mut b = [0u8; 16];
        getrandom::fill(&mut b).map_err(|_| crate::Error::CannotStart)?;
        for x in b {
            if x < 250 && s.len() < digitos {
                s.push(char::from(b'0' + x % 10));
            }
        }
    }
    Ok(s)
}

/// The same text, comparing every byte (no early exit on the first difference).
fn iguales(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

impl Entrega {
    /// A new delivery, still taking files.
    pub fn nueva(
        ahora: u64,
        caducidad: Caducidad,
        digitos: usize,
        nombres_en_recibo: bool,
    ) -> Result<Self, crate::Error> {
        Ok(Self {
            id: azar_hex(8)?,
            titulo: String::new(),
            creada: ahora,
            caducidad,
            vence: ahora + caducidad.segundos(),
            codigo: codigo_nuevo(digitos)?,
            fallos: 0,
            receptor: None,
            estado: Estado::Preparando,
            archivos: Vec::new(),
            abierta: None,
            completa: None,
            nombres_en_recibo,
        })
    }

    /// The code the sender passes on by another road.
    #[must_use]
    pub fn codigo(&self) -> &str {
        &self.codigo
    }

    /// Every file is in: the link can be given.
    pub fn lista(&mut self) {
        if self.estado == Estado::Preparando {
            self.estado = Estado::Esperando;
        }
    }

    /// Plaintext bytes in all.
    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.archivos.iter().map(|a| a.bytes).sum()
    }

    /// Its time ran out: it is closed for good. Returns whether that just happened.
    pub fn tic(&mut self, ahora: u64) -> bool {
        if self.estado.viva() && ahora >= self.vence {
            self.estado = Estado::Caducada;
            return true;
        }
        false
    }

    /// The receiver gives `codigo`, with the token its tab keeps (empty the first time).
    /// Returns what happens and, when right, the token this receiver must use from now on.
    pub fn intenta(
        &mut self,
        codigo: &str,
        token: &str,
        ahora: u64,
    ) -> Result<(Intento, Option<String>), crate::Error> {
        self.tic(ahora);
        if !matches!(self.estado, Estado::Esperando | Estado::Abierta) {
            return Ok((Intento::NoVigente, None));
        }
        if !iguales(codigo.trim(), &self.codigo) {
            self.fallos += 1;
            if self.fallos >= INTENTOS {
                self.destruye("codigo_distinto");
                return Ok((Intento::Agotada, None));
            }
            return Ok((Intento::Mal(INTENTOS - self.fallos), None));
        }
        match &self.receptor {
            Some(r) if iguales(token, r) => Ok((Intento::Bien, Some(r.clone()))),
            Some(_) => {
                self.destruye("dispositivo_nuevo");
                Ok((Intento::OtroDispositivo, None))
            }
            None => {
                let t = azar_hex(16)?;
                self.receptor = Some(t.clone());
                self.estado = Estado::Abierta;
                self.abierta = Some(ahora);
                Ok((Intento::Bien, Some(t)))
            }
        }
    }

    /// The receiver says file `i` is whole, with this root. A different root is not a receipt
    /// (the file goes again); the last confirmed file spends the link.
    pub fn recibido(&mut self, i: usize, raiz: &str, ahora: u64) -> bool {
        if self.estado != Estado::Abierta {
            return false;
        }
        let Some(a) = self.archivos.get_mut(i) else {
            return false;
        };
        if !iguales(raiz, &a.raiz) {
            return false;
        }
        a.recibido.get_or_insert(ahora);
        if self.archivos.iter().all(|a| a.recibido.is_some()) {
            self.completa = Some(ahora);
            self.estado = Estado::Entregada;
        }
        true
    }

    /// The sender withdraws it.
    pub fn revoca(&mut self) {
        if self.estado.viva() {
            self.estado = Estado::Revocada;
        }
    }

    /// Destroyed for an anomaly (`motivo`, a short key: `codigo_distinto`, `dispositivo_nuevo`,
    /// `manipulada`…). Nothing more can be opened; the program wipes the keys and the files.
    pub fn destruye(&mut self, motivo: &str) {
        if self.estado.viva() {
            self.estado = Estado::Destruida(motivo.chars().take(40).collect());
        }
    }

    /// The text both sides sign: the delivery as it ended. `receptor` is the receiver's own
    /// signature over this same text without it (its public key and signature, base64), added
    /// when it gave one. Keys in a fixed order (serde_json sorts them), so the same delivery
    /// always gives the same bytes.
    #[must_use]
    pub fn recibo(&self, receptor: Option<(&str, &str)>) -> String {
        let archivos: Vec<Value> = self
            .archivos
            .iter()
            .map(|a| {
                let mut v = json!({
                    "bytes": a.bytes,
                    "trozos": a.trozos,
                    "sha256": a.sha256,
                    "raiz_arbol_sha256": a.raiz,
                    "recibido": a.recibido,
                });
                if self.nombres_en_recibo {
                    v["nombre"] = json!(a.nombre);
                }
                v
            })
            .collect();
        let mut v = json!({
            "tipo": "guardiana-entrega-recibo",
            "version": 1,
            "entrega": self.id,
            "creada": self.creada,
            "caducidad": self.caducidad.nombre(),
            "archivos": archivos,
            "bytes": self.bytes(),
            "abierta": self.abierta,
            "descargada_completa": self.completa,
            "estado": if self.estado == Estado::Entregada { "entregado" } else { self.estado.nombre() },
            "abierto_descargado_completo": self.completa.is_some(),
            // Said in the receipt itself, so nobody reads more into it.
            "direccion_ip": "nunca se registra",
            "cifrado": "canal ECDH P-256 + clave del enlace, AES-256-GCM; cada archivo con su clave AES-256-GCM (STREAM); árbol de hashes RFC 6962",
        });
        if let Some((clave, firma)) = receptor {
            v["receptor"] = json!({ "clave_p256": clave, "firma": firma });
        }
        v.to_string()
    }

    /// The page's view of it (never the files' keys; the code only to its sender's screen).
    #[must_use]
    pub fn json(&self) -> Value {
        json!({
            "id": self.id,
            "titulo": self.titulo,
            "creada": self.creada,
            "vence": self.vence,
            "caducidad": self.caducidad.nombre(),
            "estado": self.estado.nombre(),
            "motivo": match &self.estado { Estado::Destruida(m) => json!(m), _ => Value::Null },
            "codigo": if self.estado.viva() { json!(self.codigo) } else { Value::Null },
            "fallos": self.fallos,
            "quedan": INTENTOS.saturating_sub(self.fallos),
            "abierta": self.abierta,
            "completa": self.completa,
            "bytes": self.bytes(),
            "nombres_en_recibo": self.nombres_en_recibo,
            "archivos": self.archivos.iter().map(|a| json!({
                "nombre": a.nombre, "bytes": a.bytes, "recibido": a.recibido,
            })).collect::<Vec<_>>(),
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn una(caducidad: Caducidad) -> Entrega {
        let mut e = Entrega::nueva(1000, caducidad, DIGITOS, true).unwrap();
        e.archivos.push(Ficha {
            nombre: "informes/marzo.pdf".into(),
            bytes: 10,
            trozos: 1,
            sha256: "aa".into(),
            raiz: "bb".into(),
            recibido: None,
        });
        e.archivos.push(Ficha {
            nombre: "foto.jpg".into(),
            bytes: 20,
            trozos: 1,
            sha256: "cc".into(),
            raiz: "dd".into(),
            recibido: None,
        });
        e.lista();
        e
    }

    #[test]
    fn codes_have_the_length_asked_and_only_digits() {
        for (pedido, esperado) in [(0, 4), (4, 4), (6, 6), (12, 12), (40, 12)] {
            let c = codigo_nuevo(pedido).unwrap();
            assert_eq!(c.len(), esperado);
            assert!(c.bytes().all(|b| b.is_ascii_digit()));
        }
        assert_ne!(codigo_nuevo(12).unwrap(), codigo_nuevo(12).unwrap());
        let e = una(Caducidad::Horas(24));
        assert_eq!(e.id.len(), 16);
        assert_eq!(e.codigo().len(), DIGITOS);
    }

    #[test]
    fn it_expires_at_its_hour_and_then_opens_for_nobody() {
        let mut e = una(Caducidad::Horas(24));
        assert_eq!(e.vence, 1000 + 24 * 3600);
        assert!(!e.tic(1000 + 24 * 3600 - 1));
        let c = e.codigo().to_owned();
        assert!(e.tic(1000 + 24 * 3600));
        assert_eq!(e.estado, Estado::Caducada);
        assert_eq!(
            e.intenta(&c, "", 1000 + 24 * 3600 + 5).unwrap().0,
            Intento::NoVigente
        );
        assert_eq!(Caducidad::de("7d").segundos(), 7 * 24 * 3600);
        assert_eq!(Caducidad::de("7d").nombre(), "7d");
        assert_eq!(
            Caducidad::de("primera_descarga"),
            Caducidad::PrimeraDescarga
        );
        assert_eq!(Caducidad::de("otra cosa"), Caducidad::Horas(24));
        // Expiry is checked again when a code comes, even without a tick in between.
        let mut f = una(Caducidad::Horas(1));
        let c = f.codigo().to_owned();
        assert_eq!(
            f.intenta(&c, "", 1000 + 3600).unwrap().0,
            Intento::NoVigente
        );
    }

    #[test]
    fn the_link_is_one_use_and_a_second_browser_destroys_it() {
        let mut e = una(Caducidad::PrimeraDescarga);
        let c = e.codigo().to_owned();
        let (r, token) = e.intenta(&c, "", 1100).unwrap();
        assert_eq!(r, Intento::Bien);
        let token = token.unwrap();
        assert_eq!(e.estado, Estado::Abierta);
        assert_eq!(e.abierta, Some(1100));
        // The same tab comes back after a cut: same token, it resumes.
        assert_eq!(
            e.intenta(&c, &token, 1200).unwrap(),
            (Intento::Bien, Some(token.clone()))
        );
        // Files confirmed: only with their own root; the last one spends the link.
        assert!(!e.recibido(0, "otra", 1300));
        assert!(e.recibido(0, "bb", 1300));
        assert_eq!(e.estado, Estado::Abierta);
        assert!(e.recibido(1, "dd", 1400));
        assert_eq!(e.estado, Estado::Entregada);
        assert_eq!(e.completa, Some(1400));
        assert_eq!(e.intenta(&c, &token, 1500).unwrap().0, Intento::NoVigente);

        // Another delivery: a second browser with the right code after the first opened it.
        let mut f = una(Caducidad::Horas(168));
        let c = f.codigo().to_owned();
        let (_, t) = f.intenta(&c, "", 1100).unwrap();
        assert!(t.is_some());
        assert_eq!(f.intenta(&c, "", 1101).unwrap().0, Intento::OtroDispositivo);
        assert_eq!(f.estado, Estado::Destruida("dispositivo_nuevo".into()));
        assert!(!f.recibido(0, "bb", 1102));
    }

    #[test]
    fn wrong_codes_count_down_and_the_last_destroys_it() {
        let mut e = una(Caducidad::Horas(24));
        let bueno = e.codigo().to_owned();
        let malo = if bueno == "000000" {
            "111111"
        } else {
            "000000"
        };
        assert_eq!(e.intenta(malo, "", 1001).unwrap().0, Intento::Mal(2));
        assert_eq!(e.intenta("12", "", 1002).unwrap().0, Intento::Mal(1));
        assert_eq!(e.intenta(malo, "", 1003).unwrap().0, Intento::Agotada);
        assert_eq!(e.estado, Estado::Destruida("codigo_distinto".into()));
        assert_eq!(e.intenta(&bueno, "", 1004).unwrap().0, Intento::NoVigente);
        // Revoked or destroyed stays so.
        let mut f = una(Caducidad::Horas(24));
        f.revoca();
        f.destruye("x");
        assert_eq!(f.estado, Estado::Revocada);
    }

    #[test]
    fn the_receipt_is_stable_names_files_only_if_chosen_and_never_an_address() {
        let mut e = una(Caducidad::Horas(24));
        let c = e.codigo().to_owned();
        e.intenta(&c, "", 1100).unwrap();
        e.recibido(0, "bb", 1200);
        e.recibido(1, "dd", 1300);
        let r = e.recibo(None);
        assert_eq!(r, e.recibo(None), "the same bytes every time");
        let v: Value = serde_json::from_str(&r).unwrap();
        assert_eq!(v["estado"], "entregado");
        assert_eq!(v["abierto_descargado_completo"], true);
        assert_eq!(v["archivos"][0]["nombre"], "informes/marzo.pdf");
        assert_eq!(v["archivos"][1]["raiz_arbol_sha256"], "dd");
        assert_eq!(v["bytes"], 30);
        assert!(!r.contains(&c), "the code is not in the receipt");
        e.nombres_en_recibo = false;
        let sin = e.recibo(Some(("CLAVE", "FIRMA")));
        assert!(!sin.contains("marzo"));
        let v: Value = serde_json::from_str(&sin).unwrap();
        assert_eq!(v["receptor"]["firma"], "FIRMA");
        assert_eq!(v["direccion_ip"], "nunca se registra");
        // The state survives a save, the code included (the program encrypts that file).
        let guardada: Entrega = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(guardada, e);
        assert!(
            e.json()["codigo"].is_null(),
            "a finished delivery shows no code"
        );
    }
}
