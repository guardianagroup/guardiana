//! The people this computer has verified, each pinned to the keys it was verified with.
//!
//! Trust on first use, then no silent change: once a name is pinned to a pair of keys, a
//! message or a call that arrives with other keys under that name is refused and the name is
//! marked [`Estado::ClaveCambiada`]. There is no «accept anyway»: only a new short-code check
//! ([`crate::verificar`]) pins the new keys. Francisco, 15 Oct 2026: «tecnología anti
//! suplantación de identidad del máximo nivel».
//!
//! The book holds public keys and names only. Wrapping it on disk (DPAPI on Windows) is the
//! caller's job, as with [`crate::Identity::lock`].

use serde::{Deserialize, Serialize};
use vodozemac::{Curve25519PublicKey, Ed25519PublicKey};

use crate::verificar::{Trust, Verified};
use crate::Error;

/// Whether a pinned name may be talked to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "estado", rename_all = "snake_case")]
pub enum Estado {
    /// The keys seen are the keys verified.
    Verificado,
    /// Other keys arrived under this name. Blocked until a new short-code check pins them.
    ClaveCambiada {
        /// The Ed25519 key that arrived, base64, so the person can compare it out of band.
        nueva: String,
        /// Unix seconds when it was seen.
        visto_en: u64,
    },
}

/// One verified person: the name this computer's owner gave them, and their keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contacto {
    pub nombre: String,
    /// Curve25519 key, base64: the one messages are encrypted to.
    pub curve: String,
    /// Ed25519 key, base64: the one that signs.
    pub signing: String,
    /// Unix seconds of the short-code check that pinned these keys.
    pub verificado_en: u64,
    #[serde(flatten)]
    pub estado: Estado,
}

impl Contacto {
    /// The keys, decoded. A corrupt entry yields `None` and is treated as not verified.
    fn claves(&self) -> Option<(Curve25519PublicKey, Ed25519PublicKey)> {
        Some((
            Curve25519PublicKey::from_base64(&self.curve).ok()?,
            Ed25519PublicKey::from_base64(&self.signing).ok()?,
        ))
    }
}

/// The book of verified people. Empty means nobody can write to this computer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Agenda {
    #[serde(default)]
    contactos: Vec<Contacto>,
}

impl Agenda {
    /// Nobody verified yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Pin `nombre` to the device that just passed a short-code check. A name that was
    /// blocked by a changed key is unblocked by this, and only by this.
    pub fn fijar(&mut self, nombre: &str, dispositivo: Verified, ahora: u64) {
        let nuevo = Contacto {
            nombre: nombre.to_owned(),
            curve: dispositivo.curve().to_base64(),
            signing: dispositivo.signing().to_base64(),
            verificado_en: ahora,
            estado: Estado::Verificado,
        };
        match self.contactos.iter_mut().find(|c| c.nombre == nombre) {
            Some(c) => *c = nuevo,
            None => self.contactos.push(nuevo),
        }
    }

    /// Forget a name. Nothing from those keys opens afterwards.
    pub fn olvidar(&mut self, nombre: &str) -> bool {
        let antes = self.contactos.len();
        self.contactos.retain(|c| c.nombre != nombre);
        self.contactos.len() != antes
    }

    /// The pinned entry for a name.
    pub fn contacto(&self, nombre: &str) -> Option<&Contacto> {
        self.contactos.iter().find(|c| c.nombre == nombre)
    }

    /// Every pinned person, in the order they were added.
    pub fn contactos(&self) -> &[Contacto] {
        &self.contactos
    }

    /// Before talking to `nombre` with these keys: the pinned keys must be these, and the name
    /// must not be blocked. Other keys mark the name [`Estado::ClaveCambiada`] and return
    /// [`Error::IdentityChanged`]; a blocked name returns [`Error::ReverifyNeeded`] until a new
    /// short-code check pins it again. An unknown name is [`Error::IdentityChanged`] too: nothing
    /// is said to someone never verified.
    pub fn comprueba(
        &mut self,
        nombre: &str,
        curve: Curve25519PublicKey,
        signing: Ed25519PublicKey,
        ahora: u64,
    ) -> Result<(), Error> {
        let Some(c) = self.contactos.iter_mut().find(|c| c.nombre == nombre) else {
            return Err(Error::IdentityChanged);
        };
        if let Estado::ClaveCambiada { .. } = c.estado {
            return Err(Error::ReverifyNeeded);
        }
        match c.claves() {
            Some((fc, fs)) if fc == curve && fs == signing => Ok(()),
            _ => {
                c.estado = Estado::ClaveCambiada {
                    nueva: signing.to_base64(),
                    visto_en: ahora,
                };
                Err(Error::IdentityChanged)
            }
        }
    }

    /// The devices that may be written to right now: verified names only, never a blocked one.
    pub fn trust(&self) -> Trust {
        let mut trust = Trust::new();
        for c in &self.contactos {
            if c.estado != Estado::Verificado {
                continue;
            }
            if let Some((curve, signing)) = c.claves() {
                trust.insert(Verified::new(curve, signing));
            }
        }
        trust
    }

    /// The book as JSON, to be wrapped by the caller before it touches a disk.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{\"contactos\":[]}".to_owned())
    }

    /// A book from [`Agenda::to_json`]. Anything else is [`Error::CannotRestore`].
    pub fn from_json(texto: &str) -> Result<Self, Error> {
        serde_json::from_str(texto).map_err(|_| Error::CannotRestore)
    }
}

/// The safety number two people compare: sixty digits in twelve groups of five, the same on
/// both computers whichever order the keys are given in. It is derived from both Ed25519 keys
/// (SHA-256, iterated), so it changes if either key changes. It is not a secret.
pub fn huella(a: Ed25519PublicKey, b: Ed25519PublicKey) -> String {
    use sha2::{Digest as _, Sha256};
    let (primera, segunda) = if a.as_bytes() <= b.as_bytes() {
        (a, b)
    } else {
        (b, a)
    };
    let mut h = Sha256::new();
    h.update(b"GUARDIANA_SECURE_HUELLA_V1");
    h.update(primera.as_bytes());
    h.update(segunda.as_bytes());
    let mut digest = h.finalize();
    for _ in 0..1024 {
        let mut h = Sha256::new();
        h.update(digest);
        h.update(primera.as_bytes());
        h.update(segunda.as_bytes());
        digest = h.finalize();
    }
    // 12 groups of 5 decimal digits from 12 × 4 bytes (mod 100000, as the Signal numbers do):
    // the 32 bytes give eight groups, one more hash gives the other four.
    let extra = Sha256::digest(digest);
    let grupos: Vec<String> = digest
        .chunks(4)
        .chain(extra.chunks(4).take(4))
        .map(|c| {
            let n = u32::from_be_bytes([c[0], c[1], c[2], c[3]]);
            format!("{:05}", n % 100_000)
        })
        .collect();
    grupos.join(" ")
}

/// The short form of one key for a screen: the first and last four base64 characters.
pub fn huella_corta(k: Ed25519PublicKey) -> String {
    let b = k.to_base64();
    if b.len() <= 12 {
        return b;
    }
    format!("{}…{}", &b[..4], &b[b.len() - 4..])
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::Identity;

    #[test]
    fn a_changed_key_blocks_the_name_until_it_is_pinned_again() {
        let marta = Identity::new();
        let mut agenda = Agenda::new();
        agenda.fijar(
            "Marta",
            Verified::new(marta.public_key(), marta.signing_key()),
            1_000,
        );
        assert!(agenda
            .comprueba("Marta", marta.public_key(), marta.signing_key(), 1_001)
            .is_ok());
        assert!(agenda.trust().allows(marta.public_key()));

        let impostor = Identity::new();
        assert_eq!(
            agenda
                .comprueba(
                    "Marta",
                    impostor.public_key(),
                    impostor.signing_key(),
                    1_002
                )
                .unwrap_err(),
            Error::IdentityChanged
        );
        assert!(matches!(
            agenda.contacto("Marta").unwrap().estado,
            Estado::ClaveCambiada {
                visto_en: 1_002,
                ..
            }
        ));
        // Blocked: not even the old keys talk, and the trust no longer allows them.
        assert_eq!(
            agenda
                .comprueba("Marta", marta.public_key(), marta.signing_key(), 1_003)
                .unwrap_err(),
            Error::ReverifyNeeded
        );
        assert!(!agenda.trust().allows(marta.public_key()));
        // Only a new short-code check pins again.
        agenda.fijar(
            "Marta",
            Verified::new(impostor.public_key(), impostor.signing_key()),
            2_000,
        );
        assert!(agenda
            .comprueba(
                "Marta",
                impostor.public_key(),
                impostor.signing_key(),
                2_001
            )
            .is_ok());
        assert_eq!(
            agenda
                .comprueba("Nadie", marta.public_key(), marta.signing_key(), 2_002)
                .unwrap_err(),
            Error::IdentityChanged
        );
    }

    #[test]
    fn the_book_survives_json_and_a_corrupt_text_does_not_restore() {
        let a = Identity::new();
        let mut agenda = Agenda::new();
        agenda.fijar("A", Verified::new(a.public_key(), a.signing_key()), 5);
        let json = agenda.to_json();
        assert!(json.contains("\"nombre\":\"A\""));
        assert_eq!(Agenda::from_json(&json).unwrap(), agenda);
        assert_eq!(Agenda::from_json("{x").unwrap_err(), Error::CannotRestore);
        assert!(agenda.olvidar("A"));
        assert!(!agenda.olvidar("A"));
    }

    #[test]
    fn the_safety_number_is_the_same_both_ways_and_changes_with_a_key() {
        let a = Identity::new();
        let b = Identity::new();
        let ab = huella(a.signing_key(), b.signing_key());
        assert_eq!(ab, huella(b.signing_key(), a.signing_key()));
        assert_eq!(ab.len(), 12 * 5 + 11, "{ab}");
        assert!(ab
            .split(' ')
            .all(|g| g.len() == 5 && g.chars().all(|c| c.is_ascii_digit())));
        let c = Identity::new();
        assert_ne!(ab, huella(a.signing_key(), c.signing_key()));
        let corta = huella_corta(a.signing_key());
        assert!(corta.contains('…') && corta.chars().count() == 9, "{corta}");
    }
}
