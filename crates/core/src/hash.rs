//! The hash chain of the ledger (brief §3):
//! `row_hash = sha256(prev_hash || fields)`.
//!
//! Fields are encoded unambiguously: each one as a 4-byte big-endian length
//! followed by its UTF-8 bytes, in the fixed order documented on
//! [`HashInput`]. The optional fields at the end (program, server, disguise) enter only when the
//! last of them present is not empty, each bringing the ones before it. Anyone can recompute a
//! row with only this description,
//! which is what `guardiana ledger --check` and `docs/VERIFY.md` rely on.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

/// A SHA-256 digest. Shown and exported as 64 lowercase hex characters.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hash(pub [u8; 32]);

impl Hash {
    /// SHA-256 of arbitrary bytes.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        let digest = Sha256::digest(bytes);
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        Self(out)
    }

    /// Lowercase hexadecimal form.
    #[must_use]
    pub fn to_hex(self) -> String {
        let mut s = String::with_capacity(64);
        for b in self.0 {
            s.push_str(&format!("{b:02x}"));
        }
        s
    }

    /// Parse the hexadecimal form produced by [`Hash::to_hex`].
    pub fn from_hex(s: &str) -> Result<Self> {
        if s.len() != 64 {
            return Err(Error::BadHash(s.to_owned()));
        }
        let mut out = [0u8; 32];
        for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
            let pair = std::str::from_utf8(chunk).map_err(|_| Error::BadHash(s.to_owned()))?;
            out[i] = u8::from_str_radix(pair, 16).map_err(|_| Error::BadHash(s.to_owned()))?;
        }
        Ok(Self(out))
    }

    /// Build from the raw 32 bytes stored in a BLOB column.
    pub fn from_bytes(b: &[u8]) -> Result<Self> {
        let arr: [u8; 32] = b
            .try_into()
            .map_err(|_| Error::BadHash(format!("{} bytes", b.len())))?;
        Ok(Self(arr))
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hash({})", self.to_hex())
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl Serialize for Hash {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Hash {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Self::from_hex(&s).map_err(serde::de::Error::custom)
    }
}

/// The fields that enter a row hash, in this exact order.
///
/// `ts` and `rule_id` are hashed as decimal text (`rule_id` empty when
/// absent); `signals_json` is hashed exactly as stored, byte for byte.
#[derive(Debug, Clone, Copy)]
pub struct HashInput<'a> {
    /// Unix time in milliseconds.
    pub ts: i64,
    /// Device identifier.
    pub device_id: &'a str,
    /// IP the query came from.
    pub client_ip: &'a str,
    /// Queried name.
    pub qname: &'a str,
    /// Query type (`A`, `AAAA`, ...).
    pub qtype: &'a str,
    /// Category text (`rastreador`, ...).
    pub category: &'a str,
    /// Which list produced the category, or empty.
    pub list_source: &'a str,
    /// JSON array of signal names, as stored.
    pub signals_json: &'a str,
    /// Verdict text (`observado`, ...).
    pub verdict: &'a str,
    /// Who decided (`nadie`, ...).
    pub decided_by: &'a str,
    /// Rule that caused the verdict, if any.
    pub rule_id: Option<i64>,
    /// The program that asked, as stored JSON, or empty when it is not known.
    ///
    /// Only Windows can say this today, and only for this computer, so most rows will never have
    /// it. It is hashed **only when it is there**: a row without it hashes exactly as it did
    /// before this field existed, and every ledger written until 20 September 2026 still verifies.
    pub process_json: &'a str,
    /// The server that answered, as `kind|country|asn|base` ([`crate::ServerInfo::hash_text`]),
    /// or empty (decision 203). Never an address.
    ///
    /// Hashed **only when it is there**, after `process_json`, which is then hashed too, even
    /// empty, so the two optional fields can never be read one for the other. A row without it
    /// hashes exactly as before this field existed, and every older extract still verifies.
    pub server: &'a str,
    /// The registrable site of the tracker the name answered as (decision 205), or empty.
    ///
    /// Same rule again: hashed **only when it is there**, after `process_json` and `server`,
    /// which are then hashed too, even empty. A row without it hashes exactly as before this
    /// field existed, so every older extract still verifies.
    pub cname_target: &'a str,
}

/// Compute `sha256(prev_hash || fields)` for one ledger row.
#[must_use]
pub fn chain_hash(prev: &Hash, input: &HashInput<'_>) -> Hash {
    let mut h = Sha256::new();
    h.update(prev.0);
    let ts = input.ts.to_string();
    let rule = input.rule_id.map(|r| r.to_string()).unwrap_or_default();
    for field in [
        ts.as_str(),
        input.device_id,
        input.client_ip,
        input.qname,
        input.qtype,
        input.category,
        input.list_source,
        input.signals_json,
        input.verdict,
        input.decided_by,
        rule.as_str(),
    ] {
        let len = u32::try_from(field.len()).unwrap_or(u32::MAX);
        h.update(len.to_be_bytes());
        h.update(field.as_bytes());
    }
    // Al final y solo si hay algo: así una fila sin programa da el mismo hash que antes de que
    // este campo existiera, y las cadenas ya escritas siguen comprobando.
    // Each optional field brings the ones before it, even empty: the number of fields says which
    // ones are there, and their length prefixes keep any of them from passing for another.
    let mut opcionales: Vec<&str> = Vec::new();
    if !input.cname_target.is_empty() {
        opcionales.extend([input.process_json, input.server, input.cname_target]);
    } else if !input.server.is_empty() {
        opcionales.extend([input.process_json, input.server]);
    } else if !input.process_json.is_empty() {
        opcionales.push(input.process_json);
    }
    for field in opcionales {
        let len = u32::try_from(field.len()).unwrap_or(u32::MAX);
        h.update(len.to_be_bytes());
        h.update(field.as_bytes());
    }
    let digest = h.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    Hash(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> HashInput<'static> {
        HashInput {
            ts: 1_700_000_000_000,
            device_id: "self",
            client_ip: "127.0.0.1",
            qname: "example.com",
            qtype: "A",
            category: "desconocido",
            list_source: "",
            signals_json: "[]",
            verdict: "observado",
            decided_by: "nadie",
            rule_id: None,
            process_json: "",
            server: "",
            cname_target: "",
        }
    }

    /// Fixed vectors, computed apart from this code (Python, `hashlib`, the encoding described
    /// at the top of this file): the three shapes a row could have before decision 205 hash to
    /// exactly the bytes they hashed to then, and the new shape to what the description says.
    #[test]
    fn vectores_fijos_de_antes_y_de_ahora() {
        let anterior = Hash::of(b"ancla");
        let mut s = sample();
        assert_eq!(
            chain_hash(&anterior, &s).to_hex(),
            "f7a3065227402680872802c968b7072e4dd9078d1907cd4d920f7f0e56431c4e"
        );
        s.process_json = "{\"pid\":1}";
        assert_eq!(
            chain_hash(&anterior, &s).to_hex(),
            "7dfbe1429e141aa25272ab6104db5e838fcc530a94244e1046bf23f414ab9b0b"
        );
        let mut s = sample();
        s.server = "publico|IE|32934|dbip-2026-10";
        assert_eq!(
            chain_hash(&anterior, &s).to_hex(),
            "1d4b292862de1d02f326e0f9fa2c191441d7a1c8717eee099cd9444289ebc92b"
        );
        s.process_json = "{\"pid\":1}";
        s.cname_target = "omtrdc.net";
        assert_eq!(
            chain_hash(&anterior, &s).to_hex(),
            "ed933b76ac7998ddf78510e65b63d49e2d90418e11f8e2e5dd5f4a0b684068c6"
        );
    }

    #[test]
    fn el_disfraz_no_cambia_el_hash_de_las_filas_que_no_lo_llevan() {
        // Decisión 205: un extracto escrito antes tiene que seguir comprobando, con programa,
        // con servidor, con los dos o sin ninguno.
        let anterior = Hash::of(b"ancla");
        let mut formas = Vec::new();
        for (programa, servidor) in [
            ("", ""),
            ("{\"pid\":1}", ""),
            ("", "publico|IE|32934|dbip-2026-10"),
            ("{\"pid\":1}", "publico|IE|32934|dbip-2026-10"),
        ] {
            let mut sin = sample();
            sin.process_json = programa;
            sin.server = servidor;
            let antes = chain_hash(&anterior, &sin);
            let mut vacio = sin;
            vacio.cname_target = "";
            assert_eq!(antes, chain_hash(&anterior, &vacio), "sin disfraz, igual");
            let mut con = sin;
            con.cname_target = "omtrdc.net";
            let despues = chain_hash(&anterior, &con);
            assert_ne!(antes, despues, "con disfraz cambia");
            formas.push(antes);
            formas.push(despues);
        }
        // Ninguna de las ocho formas da el hash de otra.
        let distintas: std::collections::HashSet<_> = formas.iter().collect();
        assert_eq!(distintas.len(), formas.len());
        // El disfraz no se confunde con un servidor que se llamara igual.
        let mut como_servidor = sample();
        como_servidor.server = "omtrdc.net";
        let mut como_disfraz = sample();
        como_disfraz.cname_target = "omtrdc.net";
        assert_ne!(
            chain_hash(&anterior, &como_servidor),
            chain_hash(&anterior, &como_disfraz)
        );
    }

    #[test]
    fn el_servidor_no_cambia_el_hash_de_las_filas_que_no_lo_llevan() {
        // Un extracto escrito antes de la decisión 203 tiene que seguir comprobando después de
        // actualizar, con programa o sin él.
        let anterior = Hash::of(b"ancla");
        let sin = chain_hash(&anterior, &sample());
        let mut con = sample();
        con.server = "publico|IE|32934|dbip-2026-10";
        assert_ne!(sin, chain_hash(&anterior, &con), "con dirección cambia");
        let mut programa = sample();
        programa.process_json = "{\"pid\":1}";
        let solo_programa = chain_hash(&anterior, &programa);
        // Los dos campos opcionales no se confunden: un servidor sin programa no da el hash de
        // un programa que se llamara como el servidor.
        let mut como_programa = sample();
        como_programa.process_json = "publico|IE|32934|dbip-2026-10";
        assert_ne!(
            chain_hash(&anterior, &con),
            chain_hash(&anterior, &como_programa)
        );
        programa.server = "anycast||13335|dbip-2026-10";
        assert_ne!(solo_programa, chain_hash(&anterior, &programa));
    }

    #[test]
    fn el_programa_no_cambia_el_hash_de_las_filas_que_no_lo_llevan() {
        // La razón de que este campo vaya al final y solo cuando existe: un extracto escrito antes
        // del 20 de septiembre de 2026 tiene que seguir comprobando después de actualizar.
        let anterior = Hash::of(b"ancla");
        let sin = chain_hash(&anterior, &sample());
        let mut con = sample();
        con.process_json = "{\"pid\":1234,\"nombre\":\"claude.exe\"}";
        assert_ne!(
            sin,
            chain_hash(&anterior, &con),
            "con programa tiene que cambiar"
        );
        let mut vacio = sample();
        vacio.process_json = "";
        assert_eq!(
            sin,
            chain_hash(&anterior, &vacio),
            "sin programa, el hash de siempre"
        );
    }

    #[test]
    fn hex_round_trip() {
        let h = Hash::of(b"guardiana");
        assert_eq!(Hash::from_hex(&h.to_hex()).ok(), Some(h));
        assert!(Hash::from_hex("zz").is_err());
    }

    #[test]
    fn chain_is_deterministic_and_order_sensitive() {
        let genesis = Hash::of(b"key");
        let a = chain_hash(&genesis, &sample());
        let b = chain_hash(&genesis, &sample());
        assert_eq!(a, b);
        let mut swapped = sample();
        swapped.device_id = "127.0.0.1";
        swapped.client_ip = "self";
        assert_ne!(a, chain_hash(&genesis, &swapped));
        assert_ne!(a, chain_hash(&Hash::of(b"other"), &sample()));
    }

    #[test]
    fn length_prefix_prevents_field_boundary_ambiguity() {
        // "ab" + "c" must not hash like "a" + "bc".
        let genesis = Hash::of(b"key");
        let mut x = sample();
        x.qname = "ab";
        x.qtype = "c";
        let mut y = sample();
        y.qname = "a";
        y.qtype = "bc";
        assert_ne!(chain_hash(&genesis, &x), chain_hash(&genesis, &y));
    }
}
