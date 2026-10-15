//! Short authentication strings, from [`vodozemac::sas`].
//!
//! The numbers and the MAC are the library's. The agreed info string is
//! `GUARDIANA_SECURE_SAS_V1` plus the two ephemeral public keys in byte order,
//! so both sides build the same string. This crate does not hash the keys itself.
//!
//! A [`Verified`] value is one device: its Curve25519 key and its Ed25519 key.
//! It is not a civil identity and not a professional licence. A clinic or a
//! law firm still needs a check this module does not do.

use vodozemac::sas::{EstablishedSas, Mac, Sas};
use vodozemac::{Curve25519PublicKey, Ed25519PublicKey};

use crate::{Conversation, Envelope, Error};

const INFO: &str = "GUARDIANA_SECURE_SAS_V1";

/// The short-code comparison could not be completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyError {
    /// The handshake or the MAC failed. This includes a key that was swapped.
    Failed,
    /// The person said the numbers did not match. Nothing was marked verified.
    DecimalsDiffer,
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            VerifyError::Failed => "the identity could not be verified",
            VerifyError::DecimalsDiffer => "the short codes did not match",
        })
    }
}

impl std::error::Error for VerifyError {}

/// One side of a short-code check, before the other side's ephemeral key arrives.
pub struct Inicio {
    sas: Sas,
}

impl Inicio {
    /// A new ephemeral key. It is not the identity key.
    pub fn new() -> Self {
        Self { sas: Sas::new() }
    }

    /// The ephemeral public key to send to the other side.
    pub fn public_key(&self) -> Curve25519PublicKey {
        self.sas.public_key()
    }

    /// Finish the handshake with the other side's ephemeral public key.
    pub fn establish(self, their_public: Curve25519PublicKey) -> Result<Codigo, VerifyError> {
        let established = self
            .sas
            .diffie_hellman(their_public)
            .map_err(|_| VerifyError::Failed)?;
        Ok(Codigo { established })
    }
}

impl Default for Inicio {
    fn default() -> Self {
        Self::new()
    }
}

/// A handshake that can show the short code and check a MAC.
pub struct Codigo {
    established: EstablishedSas,
}

impl Codigo {
    /// The three numbers both people compare on a channel that is not this one.
    pub fn decimals(&self) -> (u16, u16, u16) {
        self.established.bytes(&self.info()).decimals()
    }

    /// MAC of this device's two public keys. The other side checks it with [`Codigo::confirm`].
    pub fn mac(&self, signing: Ed25519PublicKey, curve: Curve25519PublicKey) -> Mac {
        self.established
            .calculate_mac(&mac_input(signing, curve), &self.info())
    }

    /// Accept the other device only if the person saw the same numbers and the MAC
    /// matches both of that device's public keys. A swapped key does not pass.
    pub fn confirm(
        &self,
        their_curve: Curve25519PublicKey,
        their_signing: Ed25519PublicKey,
        their_mac: &Mac,
        same_decimals: bool,
    ) -> Result<Verified, VerifyError> {
        if !same_decimals {
            return Err(VerifyError::DecimalsDiffer);
        }
        self.established
            .verify_mac(
                &mac_input(their_signing, their_curve),
                &self.info(),
                their_mac,
            )
            .map_err(|_| VerifyError::Failed)?;
        Ok(Verified {
            curve: their_curve,
            signing: their_signing,
        })
    }

    fn info(&self) -> String {
        let our = self.established.our_public_key();
        let their = self.established.their_public_key();
        let (first, second) = if our.as_bytes() <= their.as_bytes() {
            (our, their)
        } else {
            (their, our)
        };
        format!("{INFO}{}{}", first.to_base64(), second.to_base64())
    }
}

fn mac_input(signing: Ed25519PublicKey, curve: Curve25519PublicKey) -> String {
    format!(
        "ed25519:{}|curve25519:{}",
        signing.to_base64(),
        curve.to_base64()
    )
}

/// One device that was checked with a short code. Not a name and not a profession.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Verified {
    curve: Curve25519PublicKey,
    signing: Ed25519PublicKey,
}

impl Verified {
    /// The Curve25519 key messages are encrypted to.
    pub fn curve(&self) -> Curve25519PublicKey {
        self.curve
    }

    /// The Ed25519 key that was inside the MAC.
    pub fn signing(&self) -> Ed25519PublicKey {
        self.signing
    }
}

impl std::fmt::Debug for Verified {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Verified")
            .field("curve", &self.curve)
            .field("signing", &self.signing.to_base64())
            .finish()
    }
}

/// The devices a person has accepted. An unknown device is not allowed to receive.
#[derive(Clone, Default)]
pub struct Trust {
    peers: Vec<Verified>,
}

impl Trust {
    /// No device is accepted.
    pub fn new() -> Self {
        Self { peers: Vec::new() }
    }

    /// Remember a device that passed [`Codigo::confirm`].
    pub fn insert(&mut self, peer: Verified) {
        if !self.peers.iter().any(|known| known.signing == peer.signing) {
            self.peers.push(peer);
        }
    }

    /// Forget a device. Later sends to its Curve25519 key are refused.
    pub fn revoke(&mut self, signing: Ed25519PublicKey) -> bool {
        let before = self.peers.len();
        self.peers.retain(|peer| peer.signing != signing);
        self.peers.len() != before
    }

    /// Whether messages may be encrypted to this Curve25519 key.
    pub fn allows(&self, curve: Curve25519PublicKey) -> bool {
        self.peers.iter().any(|peer| peer.curve == curve)
    }

    /// Encrypt only if this conversation's peer is still accepted.
    /// A revoked or unknown device returns [`Error::IdentityChanged`] and nothing is encrypted.
    pub fn send(
        &self,
        conversation: &mut Conversation,
        plaintext: &[u8],
    ) -> Result<Envelope, Error> {
        if !self.allows(conversation.peer()) {
            return Err(Error::IdentityChanged);
        }
        conversation.send(conversation.peer(), plaintext)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::Identity;

    fn both() -> (crate::Identity, crate::Identity, Codigo, Codigo) {
        let alice = Identity::new();
        let bob = Identity::new();
        let alice_start = Inicio::new();
        let bob_start = Inicio::new();
        let alice_pub = alice_start.public_key();
        let bob_pub = bob_start.public_key();
        let alice_code = alice_start.establish(bob_pub).unwrap();
        let bob_code = bob_start.establish(alice_pub).unwrap();
        (alice, bob, alice_code, bob_code)
    }

    #[test]
    fn the_same_numbers_and_a_swapped_key_does_not_pass() {
        let (alice, bob, alice_code, bob_code) = both();
        assert_eq!(alice_code.decimals(), bob_code.decimals());

        let mac = bob_code.mac(bob.signing_key(), bob.public_key());
        let verified = alice_code
            .confirm(bob.public_key(), bob.signing_key(), &mac, true)
            .unwrap();
        assert_eq!(verified.curve(), bob.public_key());
        assert_eq!(verified.signing(), bob.signing_key());

        let impostor = Identity::new();
        assert_eq!(
            alice_code
                .confirm(impostor.public_key(), bob.signing_key(), &mac, true)
                .unwrap_err(),
            VerifyError::Failed
        );
        assert_eq!(
            alice_code
                .confirm(bob.public_key(), bob.signing_key(), &mac, false)
                .unwrap_err(),
            VerifyError::DecimalsDiffer
        );

        let mut trust = Trust::new();
        trust.insert(verified);
        let mut bob_recv = Identity::unlock(&bob.lock(&[9u8; 32]), &[9u8; 32]).unwrap();
        assert_eq!(bob_recv.public_key(), bob.public_key());
        let one_time = bob_recv.fresh_one_time_key().unwrap();
        let mut conversation = alice.start(verified.curve(), one_time).unwrap();
        let envelope = trust.send(&mut conversation, b"hola").unwrap();

        trust.revoke(bob.signing_key());
        assert_eq!(
            trust.send(&mut conversation, b"otro").unwrap_err(),
            Error::IdentityChanged
        );

        let (_, opened) = bob_recv.receive(alice.public_key(), &envelope).unwrap();
        assert_eq!(opened, b"hola");
    }

    #[test]
    fn a_wrong_pickle_key_does_not_restore_the_identity() {
        let alice = Identity::new();
        let locked = alice.lock(&[1u8; 32]);
        assert!(!locked.contains(&alice.signing_key().to_base64()));
        assert!(Identity::unlock(&locked, &[2u8; 32]).is_err());
        let restored = Identity::unlock(&locked, &[1u8; 32]).unwrap();
        assert_eq!(restored.public_key(), alice.public_key());
        assert_eq!(restored.signing_key(), alice.signing_key());
    }
}
