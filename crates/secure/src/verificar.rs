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
use vodozemac::{Curve25519PublicKey, Ed25519PublicKey, Ed25519Signature};

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

    /// The bytes this device signs with its Ed25519 private key.
    /// Both sides build the same bytes for the same two keys.
    pub fn transcript(&self, signing: Ed25519PublicKey, curve: Curve25519PublicKey) -> Vec<u8> {
        format!("{}|{}", self.info(), mac_input(signing, curve)).into_bytes()
    }

    /// Accept the other device only if the person saw the same numbers, the MAC
    /// matches both public keys, and `their_signature` was made by the Ed25519
    /// private key. A MAC over someone else's public keys does not pass.
    pub fn confirm(
        &self,
        their_curve: Curve25519PublicKey,
        their_signing: Ed25519PublicKey,
        their_mac: &Mac,
        their_signature: &Ed25519Signature,
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
        let transcript = self.transcript(their_signing, their_curve);
        their_signing
            .verify(&transcript, their_signature)
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

/// The bytes one device sends to the other: both public keys, the SAS MAC and
/// the Ed25519 signature. Not a plaintext message.
pub fn empaquetar(device: &crate::Identity, code: &Codigo) -> Result<Vec<u8>, VerifyError> {
    let mac = code.mac(device.signing_key(), device.public_key());
    if mac.as_bytes().len() != 32 {
        return Err(VerifyError::Failed);
    }
    let signature = device.sign(&code.transcript(device.signing_key(), device.public_key()));
    let mut out = Vec::with_capacity(160);
    out.extend_from_slice(device.public_key().as_bytes());
    out.extend_from_slice(device.signing_key().as_bytes());
    out.extend_from_slice(mac.as_bytes());
    out.extend_from_slice(&signature.to_bytes());
    Ok(out)
}

/// A received proof that is not verified until the person compares the numbers.
/// Nothing in this type reads the other side's claim of what the numbers were.
pub struct Pendiente {
    code: Codigo,
    curve: Curve25519PublicKey,
    signing: Ed25519PublicKey,
    mac: Mac,
    signature: Ed25519Signature,
}

impl Pendiente {
    /// Hold a proof that arrived from the other device. This does not verify it.
    pub fn sostener(code: Codigo, bytes: &[u8]) -> Result<Self, VerifyError> {
        if bytes.len() != 160 {
            return Err(VerifyError::Failed);
        }
        let curve =
            Curve25519PublicKey::from_slice(&bytes[..32]).map_err(|_| VerifyError::Failed)?;
        let mut signing_bytes = [0u8; 32];
        signing_bytes.copy_from_slice(&bytes[32..64]);
        let signing =
            Ed25519PublicKey::from_slice(&signing_bytes).map_err(|_| VerifyError::Failed)?;
        let mac = Mac::from_slice(&bytes[64..96]);
        let signature =
            Ed25519Signature::from_slice(&bytes[96..]).map_err(|_| VerifyError::Failed)?;
        Ok(Self {
            code,
            curve,
            signing,
            mac,
            signature,
        })
    }

    /// The three numbers this device computed. Show these. Do not replace them
    /// with numbers that arrived on the same socket.
    pub fn numeros(&self) -> (u16, u16, u16) {
        self.code.decimals()
    }

    /// The person compared [`Pendiente::numeros`] with the other person, out of band.
    /// `false` verifies nothing.
    pub fn la_persona_dice(self, coinciden: bool) -> Result<Verified, VerifyError> {
        self.code.confirm(
            self.curve,
            self.signing,
            &self.mac,
            &self.signature,
            coinciden,
        )
    }
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

    pub(crate) fn from_public_keys(curve: Curve25519PublicKey, signing: Ed25519PublicKey) -> Self {
        Self { curve, signing }
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

/// The devices a person has accepted. An unknown or revoked device is not
/// allowed to receive. Revocation stays in this value until it is saved and
/// loaded again; a plain [`Trust::insert`] does not undo it.
#[derive(Clone, Default)]
pub struct Trust {
    peers: Vec<Verified>,
    revoked: Vec<Ed25519PublicKey>,
}

impl Trust {
    /// No device is accepted.
    pub fn new() -> Self {
        Self {
            peers: Vec::new(),
            revoked: Vec::new(),
        }
    }

    /// Remember a device that passed [`Codigo::confirm`].
    /// A revoked signing key is not added again. A new comparison uses [`Trust::aceptar`].
    pub fn insert(&mut self, peer: Verified) {
        if self.revoked.contains(&peer.signing) {
            return;
        }
        if !self.peers.iter().any(|known| known.signing == peer.signing) {
            self.peers.push(peer);
        }
    }

    /// Trust a device after a new short-code comparison, including one that
    /// was revoked. This is a deliberate act. [`Trust::insert`] will not do it.
    pub fn aceptar(&mut self, peer: Verified) {
        self.revoked.retain(|key| *key != peer.signing);
        if !self.peers.iter().any(|known| known.signing == peer.signing) {
            self.peers.push(peer);
        }
    }

    /// Forget a device. Later sends to its Curve25519 key are refused, and
    /// [`Trust::insert`] will not put it back.
    pub fn revoke(&mut self, signing: Ed25519PublicKey) -> bool {
        if !self.revoked.contains(&signing) {
            self.revoked.push(signing);
        }
        let before = self.peers.len();
        self.peers.retain(|peer| peer.signing != signing);
        self.peers.len() != before
    }

    /// Whether this signing key was revoked in this list.
    pub fn is_revoked(&self, signing: Ed25519PublicKey) -> bool {
        self.revoked.contains(&signing)
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

    /// Open a later message only if this conversation's peer is still accepted.
    pub fn open(
        &self,
        conversation: &mut Conversation,
        envelope: &Envelope,
    ) -> Result<Vec<u8>, Error> {
        if !self.allows(conversation.peer()) {
            return Err(Error::IdentityChanged);
        }
        conversation.open(envelope)
    }

    pub(crate) fn restore(peers: Vec<Verified>, revoked: Vec<Ed25519PublicKey>) -> Self {
        let peers = peers
            .into_iter()
            .filter(|peer| !revoked.contains(&peer.signing))
            .collect();
        Self { peers, revoked }
    }

    pub(crate) fn peers(&self) -> &[Verified] {
        &self.peers
    }

    pub(crate) fn revoked_keys(&self) -> &[Ed25519PublicKey] {
        &self.revoked
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

    fn accept(code: &Codigo, device: &Identity) -> Verified {
        let mac = code.mac(device.signing_key(), device.public_key());
        let signature = device.sign(&code.transcript(device.signing_key(), device.public_key()));
        code.confirm(
            device.public_key(),
            device.signing_key(),
            &mac,
            &signature,
            true,
        )
        .unwrap()
    }

    #[test]
    fn the_same_numbers_and_a_swapped_key_does_not_pass() {
        let (alice, bob, alice_code, bob_code) = both();
        assert_eq!(alice_code.decimals(), bob_code.decimals());

        let verified = accept(&alice_code, &bob);
        assert_eq!(verified.curve(), bob.public_key());
        assert_eq!(verified.signing(), bob.signing_key());

        let mac = bob_code.mac(bob.signing_key(), bob.public_key());
        let signature = bob.sign(&bob_code.transcript(bob.signing_key(), bob.public_key()));
        let impostor = Identity::new();
        assert_eq!(
            alice_code
                .confirm(
                    impostor.public_key(),
                    bob.signing_key(),
                    &mac,
                    &signature,
                    true,
                )
                .unwrap_err(),
            VerifyError::Failed
        );
        assert_eq!(
            alice_code
                .confirm(bob.public_key(), bob.signing_key(), &mac, &signature, false)
                .unwrap_err(),
            VerifyError::DecimalsDiffer
        );

        let mut alice_trust = Trust::new();
        alice_trust.insert(verified);
        let mut bob_recv = Identity::unlock(&bob.lock(&[9u8; 32]), &[9u8; 32]).unwrap();
        let one_time = bob_recv.fresh_one_time_key().unwrap();
        let mut conversation = alice.start(verified.curve(), one_time).unwrap();
        let envelope = alice_trust.send(&mut conversation, b"hola").unwrap();

        alice_trust.revoke(bob.signing_key());
        assert_eq!(
            alice_trust.send(&mut conversation, b"otro").unwrap_err(),
            Error::IdentityChanged
        );

        let mut bob_trust = Trust::new();
        assert_eq!(
            bob_recv
                .receive(&bob_trust, alice.public_key(), &envelope)
                .unwrap_err(),
            Error::IdentityChanged
        );
        bob_trust.insert(accept(&bob_code, &alice));
        let (mut bob_side, opened) = bob_recv
            .receive(&bob_trust, alice.public_key(), &envelope)
            .unwrap();
        assert_eq!(opened, b"hola");

        let reply = bob_trust.send(&mut bob_side, b"recibido").unwrap();
        alice_trust.aceptar(verified);
        assert_eq!(
            alice_trust.open(&mut conversation, &reply).unwrap(),
            b"recibido"
        );
        alice_trust.revoke(bob.signing_key());
        let again = bob_trust.send(&mut bob_side, b"otra").unwrap();
        assert_eq!(
            alice_trust.open(&mut conversation, &again).unwrap_err(),
            Error::IdentityChanged
        );
    }

    #[test]
    fn a_mac_of_someone_elses_keys_does_not_prove_possession() {
        let bob = Identity::new();
        let mallory = Identity::new();
        let alice_start = Inicio::new();
        let mallory_start = Inicio::new();
        let alice_pub = alice_start.public_key();
        let mallory_pub = mallory_start.public_key();
        let alice_code = alice_start.establish(mallory_pub).unwrap();
        let mallory_code = mallory_start.establish(alice_pub).unwrap();

        let mac_of_bob = mallory_code.mac(bob.signing_key(), bob.public_key());
        let forged = mallory.sign(&mallory_code.transcript(bob.signing_key(), bob.public_key()));
        assert_eq!(
            alice_code
                .confirm(
                    bob.public_key(),
                    bob.signing_key(),
                    &mac_of_bob,
                    &forged,
                    true,
                )
                .unwrap_err(),
            VerifyError::Failed
        );

        let own = accept(&alice_code, &mallory);
        assert_eq!(own.signing(), mallory.signing_key());
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

    /// The proof is built on one side's [`Codigo`] and checked on the other side's.
    fn prove(
        code: &Codigo,
        device: &Identity,
    ) -> (vodozemac::sas::Mac, vodozemac::Ed25519Signature) {
        let mac = code.mac(device.signing_key(), device.public_key());
        let signature = device.sign(&code.transcript(device.signing_key(), device.public_key()));
        (mac, signature)
    }

    #[test]
    fn each_side_checks_the_proof_the_other_made() {
        let (alice, bob, alice_code, bob_code) = both();
        assert_eq!(alice_code.decimals(), bob_code.decimals());

        let (bob_mac, bob_signature) = prove(&bob_code, &bob);
        let (alice_mac, alice_signature) = prove(&alice_code, &alice);

        let bob_for_alice = alice_code
            .confirm(
                bob.public_key(),
                bob.signing_key(),
                &bob_mac,
                &bob_signature,
                true,
            )
            .unwrap();
        let alice_for_bob = bob_code
            .confirm(
                alice.public_key(),
                alice.signing_key(),
                &alice_mac,
                &alice_signature,
                true,
            )
            .unwrap();
        assert_eq!(bob_for_alice.curve(), bob.public_key());
        assert_eq!(bob_for_alice.signing(), bob.signing_key());
        assert_eq!(alice_for_bob.curve(), alice.public_key());
        assert_eq!(alice_for_bob.signing(), alice.signing_key());
    }

    #[test]
    fn a_received_proof_does_not_survive_a_substituted_key() {
        let (_, bob, alice_code, bob_code) = both();
        let (bob_mac, bob_signature) = prove(&bob_code, &bob);
        let impostor = Identity::new();
        assert_eq!(
            alice_code
                .confirm(
                    impostor.public_key(),
                    bob.signing_key(),
                    &bob_mac,
                    &bob_signature,
                    true,
                )
                .unwrap_err(),
            VerifyError::Failed
        );
        assert_eq!(
            alice_code
                .confirm(
                    bob.public_key(),
                    impostor.signing_key(),
                    &bob_mac,
                    &bob_signature,
                    true,
                )
                .unwrap_err(),
            VerifyError::Failed
        );
    }

    #[test]
    fn a_proof_from_one_session_does_not_open_another() {
        let (_, bob, alice_code, bob_code) = both();
        let (bob_mac, bob_signature) = prove(&bob_code, &bob);

        let alice_start = Inicio::new();
        let bob_start = Inicio::new();
        let alice_pub = alice_start.public_key();
        let bob_pub = bob_start.public_key();
        let alice_again = alice_start.establish(bob_pub).unwrap();
        let bob_again = bob_start.establish(alice_pub).unwrap();
        let (bob_mac_again, bob_signature_again) = prove(&bob_again, &bob);

        assert_eq!(
            alice_again
                .confirm(
                    bob.public_key(),
                    bob.signing_key(),
                    &bob_mac,
                    &bob_signature,
                    true,
                )
                .unwrap_err(),
            VerifyError::Failed
        );
        assert_eq!(
            alice_code
                .confirm(
                    bob.public_key(),
                    bob.signing_key(),
                    &bob_mac_again,
                    &bob_signature_again,
                    true,
                )
                .unwrap_err(),
            VerifyError::Failed
        );
        assert_eq!(
            alice_again
                .confirm(
                    bob.public_key(),
                    bob.signing_key(),
                    &bob_mac_again,
                    &bob_signature_again,
                    true,
                )
                .unwrap()
                .signing(),
            bob.signing_key()
        );
    }

    #[test]
    fn saying_the_numbers_differ_verifies_nothing_and_revocation_survives() {
        let (alice, bob, alice_code, bob_code) = both();
        let packed = empaquetar(&bob, &bob_code).unwrap();
        let pending = Pendiente::sostener(alice_code, &packed).unwrap();
        assert_eq!(pending.numeros(), bob_code.decimals());
        assert_eq!(
            pending.la_persona_dice(false).unwrap_err(),
            VerifyError::DecimalsDiffer
        );

        let verified = accept(&bob_code, &alice);
        let mut trust = Trust::new();
        trust.insert(verified);
        assert!(trust.revoke(alice.signing_key()));
        trust.insert(verified);
        assert!(!trust.allows(alice.public_key()));
        let restored = Trust::restore(trust.peers().to_vec(), trust.revoked_keys().to_vec());
        assert!(restored.is_revoked(alice.signing_key()));
        assert!(!restored.allows(alice.public_key()));
        let mut again = restored;
        again.aceptar(verified);
        assert!(again.allows(alice.public_key()));
        assert!(!again.is_revoked(alice.signing_key()));
    }
}
