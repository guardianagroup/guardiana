//! The record that is kept when something is seen during a conversation, so it can be shown
//! later as evidence. The owner, 15 Oct 2026: «en caso de intento de espionaje o sabotaje debe
//! quedar todo guardado … en PDF y más formatos … como prueba».
//!
//! Every entry carries the hash of the one before it (`hash_n = sha256(hash_{n-1} ‖ fields)`),
//! as GUARDIANA's own book does, and the whole chain is signed with this computer's Ed25519
//! identity key when it is exported. Change one field, drop one line or reorder two and
//! [`Prueba::verificar`] fails. JSON is the source of truth; the CSV is derived from it. No
//! entry ever holds a message, a file's contents or an address.

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use vodozemac::{Ed25519PublicKey, Ed25519Signature};

use crate::monitor::Nivel;
use crate::{Error, Identity};

const DOMINIO: &str = "GUARDIANA_SECURE_PRUEBA_V1";

/// One thing that was seen, with what proves it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registro {
    /// Position in the chain, from 1.
    pub n: u64,
    /// Unix seconds.
    pub cuando: u64,
    /// How sure the entry is.
    pub nivel: Nivel,
    /// What happened, in the person's language.
    pub texto: String,
    /// The program seen doing it, when one was (its file name).
    pub proceso: Option<String>,
    /// That program's path.
    pub ruta: Option<String>,
    /// SHA-256 of that program's executable, hex, when it could be read.
    pub sha256_exe: Option<String>,
    /// Which Windows source said so (`ConsentStore`, `ETW FileIO`, `WFP`…), with the value seen.
    pub evidencia: Option<String>,
    /// SHA-256 over the previous hash and this entry's fields, hex.
    pub hash: String,
}

/// What an entry is before it is chained.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hecho {
    /// What happened, in the person's language.
    pub texto: String,
    /// The program seen doing it, when one was.
    pub proceso: Option<String>,
    /// That program's path.
    pub ruta: Option<String>,
    /// SHA-256 of its executable, hex.
    pub sha256_exe: Option<String>,
    /// Which source said so, with the value seen.
    pub evidencia: Option<String>,
}

/// The chain of one session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cadena {
    /// A name for the session (the Olm session id, say). Not an address.
    pub sesion: String,
    /// Unix seconds when the session started.
    pub inicio: u64,
    /// The entries, in order.
    pub registros: Vec<Registro>,
}

fn hash_de(anterior: &str, n: u64, cuando: u64, nivel: Nivel, h: &Hecho) -> String {
    // One deterministic text per entry: the fields as a JSON array, in a fixed order.
    let campos = serde_json::to_string(&(
        n,
        cuando,
        nivel,
        &h.texto,
        &h.proceso,
        &h.ruta,
        &h.sha256_exe,
        &h.evidencia,
    ))
    .unwrap_or_default();
    let mut s = Sha256::new();
    s.update(DOMINIO.as_bytes());
    s.update(anterior.as_bytes());
    s.update(campos.as_bytes());
    hex(&s.finalize())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

impl Cadena {
    /// An empty chain for a session.
    pub fn new(sesion: impl Into<String>, inicio: u64) -> Self {
        Self {
            sesion: sesion.into(),
            inicio,
            registros: Vec::new(),
        }
    }

    /// The last hash, or the session's own name for the first entry.
    pub fn ultimo_hash(&self) -> String {
        self.registros.last().map_or_else(
            || format!("{}:{}", self.sesion, self.inicio),
            |r| r.hash.clone(),
        )
    }

    /// Add an entry, chained to the one before.
    pub fn anota(&mut self, cuando: u64, nivel: Nivel, hecho: Hecho) {
        let n = self.registros.len() as u64 + 1;
        let hash = hash_de(&self.ultimo_hash(), n, cuando, nivel, &hecho);
        self.registros.push(Registro {
            n,
            cuando,
            nivel,
            texto: hecho.texto,
            proceso: hecho.proceso,
            ruta: hecho.ruta,
            sha256_exe: hecho.sha256_exe,
            evidencia: hecho.evidencia,
            hash,
        });
    }

    /// Recompute every hash. `Err` names nothing: the chain is simply not the one that was made.
    pub fn comprueba(&self) -> Result<(), Error> {
        let mut anterior = format!("{}:{}", self.sesion, self.inicio);
        for (i, r) in self.registros.iter().enumerate() {
            let n = i as u64 + 1;
            let hecho = Hecho {
                texto: r.texto.clone(),
                proceso: r.proceso.clone(),
                ruta: r.ruta.clone(),
                sha256_exe: r.sha256_exe.clone(),
                evidencia: r.evidencia.clone(),
            };
            if r.n != n || hash_de(&anterior, n, r.cuando, r.nivel, &hecho) != r.hash {
                return Err(Error::CannotOpen);
            }
            anterior = r.hash.clone();
        }
        Ok(())
    }

    /// The chain as CSV (header, one line per entry), derived from the same fields the hashes
    /// cover. Fields with a comma, a quote or a newline are quoted.
    pub fn to_csv(&self) -> String {
        let q = |s: &str| {
            if s.contains([',', '"', '\n', '\r']) {
                format!("\"{}\"", s.replace('"', "\"\""))
            } else {
                s.to_owned()
            }
        };
        let o = |s: &Option<String>| q(s.as_deref().unwrap_or(""));
        let mut out = String::from("n,cuando,nivel,texto,proceso,ruta,sha256_exe,evidencia,hash\n");
        for r in &self.registros {
            let nivel = serde_json::to_string(&r.nivel).unwrap_or_default();
            out.push_str(&format!(
                "{},{},{},{},{},{},{},{},{}\n",
                r.n,
                r.cuando,
                nivel.trim_matches('"'),
                q(&r.texto),
                o(&r.proceso),
                o(&r.ruta),
                o(&r.sha256_exe),
                o(&r.evidencia),
                r.hash
            ));
        }
        out
    }
}

/// A chain signed by the computer that made it: what leaves as a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prueba {
    /// Format version.
    pub version: u32,
    /// The GUARDIANA version that wrote it.
    pub programa: String,
    /// The chain itself.
    pub cadena: Cadena,
    /// Unix seconds when it was signed.
    pub firmada_en: u64,
    /// How many entries the signature covers.
    pub registros: u64,
    /// The last hash, repeated here so a printed sheet can show it.
    pub hash_final: String,
    /// The signing computer's Ed25519 key, base64. Compare it with the key on its card.
    pub firmante: String,
    /// Ed25519 signature over `version|programa|sesion|inicio|firmada_en|registros|hash_final`.
    pub firma: String,
}

fn transcript(p: &Prueba) -> String {
    format!(
        "{DOMINIO}|{}|{}|{}|{}|{}|{}|{}",
        p.version,
        p.programa,
        p.cadena.sesion,
        p.cadena.inicio,
        p.firmada_en,
        p.registros,
        p.hash_final
    )
}

impl Prueba {
    /// Sign a chain with this computer's identity.
    pub fn firmar(identity: &Identity, cadena: Cadena, programa: &str, ahora: u64) -> Self {
        let mut p = Self {
            version: 1,
            programa: programa.to_owned(),
            registros: cadena.registros.len() as u64,
            hash_final: cadena.ultimo_hash(),
            cadena,
            firmada_en: ahora,
            firmante: identity.signing_key().to_base64(),
            firma: String::new(),
        };
        p.firma = identity.sign(transcript(&p).as_bytes()).to_base64();
        p
    }

    /// Check the chain and the signature. Passing means: this exact list of entries, in this
    /// order, was signed by the holder of `firmante`'s private key. It does not say who that is;
    /// the card or the safety number does.
    pub fn verificar(&self) -> Result<Ed25519PublicKey, Error> {
        self.cadena.comprueba()?;
        if self.registros != self.cadena.registros.len() as u64
            || self.hash_final != self.cadena.ultimo_hash()
        {
            return Err(Error::CannotOpen);
        }
        let clave = Ed25519PublicKey::from_base64(&self.firmante).map_err(|_| Error::CannotOpen)?;
        let firma = Ed25519Signature::from_base64(&self.firma).map_err(|_| Error::CannotOpen)?;
        clave
            .verify(transcript(self).as_bytes(), &firma)
            .map_err(|_| Error::CannotOpen)?;
        Ok(clave)
    }

    /// The file: JSON, the source of truth.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Read a file back. Nothing is trusted until [`Prueba::verificar`] passes.
    pub fn from_json(texto: &str) -> Result<Self, Error> {
        serde_json::from_str(texto).map_err(|_| Error::CannotRestore)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn visto(texto: &str, proceso: Option<&str>) -> Hecho {
        Hecho {
            texto: texto.to_owned(),
            proceso: proceso.map(str::to_owned),
            ruta: proceso.map(|p| format!("C:\\x\\{p}")),
            sha256_exe: None,
            evidencia: Some("ConsentStore LastUsedTimeStart".to_owned()),
        }
    }

    #[test]
    fn a_signed_chain_verifies_and_any_change_breaks_it() {
        let yo = Identity::new();
        let mut c = Cadena::new("sesion-1", 1_000);
        c.anota(
            1_001,
            Nivel::Observada,
            visto("grab.exe usó el micrófono", Some("grab.exe")),
        );
        c.anota(
            1_002,
            Nivel::Inusual,
            visto("conexión nueva de otro.exe", Some("otro.exe")),
        );
        c.anota(
            1_003,
            Nivel::NoVerificable,
            visto("quién leyó el portapapeles", None),
        );
        assert!(c.comprueba().is_ok());
        let p = Prueba::firmar(&yo, c.clone(), "1.0.31", 1_010);
        assert_eq!(p.verificar().unwrap(), yo.signing_key());

        let json = p.to_json();
        let leida = Prueba::from_json(&json).unwrap();
        assert_eq!(leida, p);
        assert!(leida.verificar().is_ok());

        // A changed text.
        let mut t = p.clone();
        t.cadena.registros[1].texto = "nada".to_owned();
        assert_eq!(t.verificar().unwrap_err(), Error::CannotOpen);
        // A dropped entry.
        let mut t = p.clone();
        t.cadena.registros.pop();
        assert!(t.verificar().is_err());
        // Two entries swapped.
        let mut t = p.clone();
        t.cadena.registros.swap(0, 1);
        assert!(t.verificar().is_err());
        // Another computer's key named as the signer.
        let mut t = p.clone();
        t.firmante = Identity::new().signing_key().to_base64();
        assert!(t.verificar().is_err());
        // A later signing time under the old signature.
        let mut t = p.clone();
        t.firmada_en += 1;
        assert!(t.verificar().is_err());

        let csv = c.to_csv();
        assert_eq!(csv.lines().count(), 4);
        assert!(csv.contains("observada") && csv.contains("no_verificable"));
        assert!(csv.lines().nth(1).unwrap().ends_with(&c.registros[0].hash));
        assert!(Prueba::from_json("{").is_err());
    }

    #[test]
    fn a_comma_in_a_text_is_quoted_in_the_csv() {
        let mut c = Cadena::new("s", 1);
        c.anota(2, Nivel::Observada, visto("uno, dos \"tres\"", None));
        assert!(c.to_csv().contains("\"uno, dos \"\"tres\"\"\""));
    }
}
