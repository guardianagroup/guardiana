//! GUARDIANA SECURE, the cryptographic hello.
//!
//! Two identities exchange one message with Olm, the double ratchet
//! [vodozemac](https://github.com/matrix-org/vodozemac) already implements
//! (Apache-2.0). This crate does not write a cipher and is not linked from
//! the DNS service. Keys stay inside the [`Identity`] that created them.
//!
//! [`carry`] moves one envelope across a loopback socket. The relay stores
//! the ciphertext and the sender's public key. It has no identity, so it
//! cannot open the message. A server between two countries would carry these
//! same bytes. That server is not deployed here.
//!
//! A changed identity key makes [`Conversation::send`] refuse before it
//! encrypts. That is a check, not a claim that spying was found.

use vodozemac::olm::{Account, OlmMessage, Session, SessionConfig, SessionCreationError};
use vodozemac::Curve25519PublicKey;

pub mod carry;

/// Why a SECURE operation stopped. The text never names a key or a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// No one-time key was produced.
    NoOneTimeKey,
    /// The session could not be started.
    CannotStart,
    /// The key just presented is not the one this conversation was opened with.
    /// Nothing was encrypted.
    IdentityChanged,
    /// The first message was not a pre-key message.
    NotAFirstMessage,
    /// The ciphertext could not be opened. A changed byte, a spent one-time
    /// key, and a stranger's attempt all end here, and this error does not say which.
    CannotOpen,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Error::NoOneTimeKey => "no one-time key",
            Error::CannotStart => "the session could not be started",
            Error::IdentityChanged => "the identity key is not the one that was accepted",
            Error::NotAFirstMessage => "the first message was not a pre-key message",
            Error::CannotOpen => "the message could not be opened",
        })
    }
}

impl std::error::Error for Error {}

/// One SECURE identity. The secret keys never leave this value.
pub struct Identity {
    account: Account,
}

impl std::fmt::Debug for Identity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Identity")
            .field("curve25519", &self.account.curve25519_key())
            .finish()
    }
}

impl Identity {
    /// A new identity. The keys are generated here and are not derived from a password.
    pub fn new() -> Self {
        Self {
            account: Account::new(),
        }
    }

    /// The public Curve25519 identity key. This is the key the other person accepts.
    pub fn public_key(&self) -> Curve25519PublicKey {
        self.account.curve25519_key()
    }

    /// One new one-time public key. The matching secret stays in this identity
    /// until [`Identity::receive`] consumes it.
    pub fn fresh_one_time_key(&mut self) -> Result<Curve25519PublicKey, Error> {
        self.account
            .generate_one_time_keys(1)
            .created
            .into_iter()
            .next()
            .ok_or(Error::NoOneTimeKey)
    }

    /// Start a conversation with an identity key the person has accepted, and
    /// one of that identity's one-time keys.
    pub fn start(
        &self,
        accepted: Curve25519PublicKey,
        one_time_key: Curve25519PublicKey,
    ) -> Result<Conversation, Error> {
        let session = self
            .account
            .create_outbound_session(SessionConfig::version_1(), accepted, one_time_key)
            .map_err(|_| Error::CannotStart)?;
        Ok(Conversation {
            session,
            peer: accepted,
        })
    }

    /// Open the first message. `from` must be the sender's identity key; if it
    /// is not the key inside the message, nothing is decrypted.
    pub fn receive(
        &mut self,
        from: Curve25519PublicKey,
        envelope: &Envelope,
    ) -> Result<(Conversation, Vec<u8>), Error> {
        let OlmMessage::PreKey(pre_key) = &envelope.message else {
            return Err(Error::NotAFirstMessage);
        };
        match self
            .account
            .create_inbound_session(SessionConfig::version_1(), from, pre_key)
        {
            Ok(created) => Ok((
                Conversation {
                    session: created.session,
                    peer: from,
                },
                created.plaintext,
            )),
            Err(SessionCreationError::MismatchedIdentityKey(_, _)) => Err(Error::IdentityChanged),
            Err(
                SessionCreationError::MissingOneTimeKey(_)
                | SessionCreationError::Decryption(_)
                | SessionCreationError::MismatchedSessionConfig { .. }
                | SessionCreationError::NonContributoryKey,
            ) => Err(Error::CannotOpen),
        }
    }
}

impl Default for Identity {
    fn default() -> Self {
        Self::new()
    }
}

/// A conversation bound to one accepted identity key.
pub struct Conversation {
    session: Session,
    peer: Curve25519PublicKey,
}

impl std::fmt::Debug for Conversation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Conversation")
            .field("peer", &self.peer)
            .finish()
    }
}

impl Conversation {
    /// The identity key this conversation will encrypt to.
    pub fn peer(&self) -> Curve25519PublicKey {
        self.peer
    }

    /// Encrypt `plaintext` only if `accepted` is still the peer of this conversation.
    /// A different key returns [`Error::IdentityChanged`] and does not encrypt.
    pub fn send(
        &mut self,
        accepted: Curve25519PublicKey,
        plaintext: &[u8],
    ) -> Result<Envelope, Error> {
        if accepted != self.peer {
            return Err(Error::IdentityChanged);
        }
        let message = self
            .session
            .encrypt(plaintext)
            .map_err(|_| Error::CannotOpen)?;
        Ok(Envelope { message })
    }

    /// Open a later message on this conversation.
    pub fn open(&mut self, envelope: &Envelope) -> Result<Vec<u8>, Error> {
        self.session
            .decrypt(&envelope.message)
            .map_err(|_| Error::CannotOpen)
    }
}

/// Ciphertext. Debug prints the length, never the bytes and never the plaintext.
pub struct Envelope {
    message: OlmMessage,
}

impl std::fmt::Debug for Envelope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (_, bytes) = self.message.to_parts();
        f.debug_struct("Envelope")
            .field("bytes", &bytes.len())
            .finish()
    }
}

impl Envelope {
    /// The message type and the ciphertext bytes, for carrying the envelope
    /// somewhere else. This crate does not carry it.
    pub fn to_wire(&self) -> (usize, Vec<u8>) {
        self.message.to_parts()
    }

    /// Rebuild an envelope from [`Envelope::to_wire`]. A byte that does not
    /// decode is [`Error::CannotOpen`].
    pub fn from_wire(message_type: usize, ciphertext: &[u8]) -> Result<Self, Error> {
        OlmMessage::from_parts(message_type, ciphertext)
            .map(|message| Self { message })
            .map_err(|_| Error::CannotOpen)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn pair() -> (Identity, Identity, Conversation, Curve25519PublicKey) {
        let alice = Identity::new();
        let mut bob = Identity::new();
        let one_time = bob.fresh_one_time_key().unwrap();
        let accepted = bob.public_key();
        let conversation = alice.start(accepted, one_time).unwrap();
        (alice, bob, conversation, accepted)
    }

    #[test]
    fn a_message_opens_only_for_the_accepted_identity() {
        let (alice, mut bob, mut alice_side, accepted) = pair();
        let envelope = alice_side.send(accepted, b"hola").unwrap();
        let (_, bytes) = envelope.to_wire();
        assert!(!bytes.windows(b"hola".len()).any(|window| window == b"hola"));

        let (mut bob_side, opened) = bob.receive(alice.public_key(), &envelope).unwrap();
        assert_eq!(opened, b"hola");

        let reply = bob_side.send(alice.public_key(), b"recibido").unwrap();
        assert_eq!(alice_side.open(&reply).unwrap(), b"recibido");
    }

    #[test]
    fn a_changed_byte_does_not_open() {
        let (alice, mut bob, mut alice_side, accepted) = pair();
        let envelope = alice_side.send(accepted, b"hola").unwrap();
        let (kind, mut bytes) = envelope.to_wire();
        let last = bytes.last_mut().unwrap();
        *last ^= 0x01;
        let tampered = Envelope::from_wire(kind, &bytes).unwrap();
        assert!(bob.receive(alice.public_key(), &tampered).is_err());
    }

    #[test]
    fn a_stranger_does_not_open_it_and_a_new_key_does_not_send() {
        let (alice, mut bob, mut alice_side, accepted) = pair();
        let envelope = alice_side.send(accepted, b"hola").unwrap();

        let mut carol = Identity::new();
        assert_eq!(
            carol.receive(alice.public_key(), &envelope).unwrap_err(),
            Error::CannotOpen
        );
        assert_eq!(
            bob.receive(carol.public_key(), &envelope).unwrap_err(),
            Error::IdentityChanged
        );

        let replacement = Identity::new();
        assert_eq!(
            alice_side
                .send(replacement.public_key(), b"otro")
                .unwrap_err(),
            Error::IdentityChanged
        );
        let still = alice_side.send(accepted, b"sigue").unwrap();
        let (_, opened) = bob.receive(alice.public_key(), &still).unwrap();
        assert_eq!(opened, b"sigue");
    }
}
