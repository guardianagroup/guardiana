//! HumanVault — the encrypted container on the user's machine (concept documents of September
//! 2026, DECISIONES #103 and #126). No server, no copy of ours, no back door: the master key is
//! derived from the owner's password (or the 24 recovery words) on the device and never leaves it.
//!
//! Two promises this crate is built around (DECISIONES #103): cancelling a subscription never
//! blocks decryption (nothing here checks a licence), and the format is open and readable by an
//! independent program from day one (`crates/vault-reader`, `docs/BOVEDA.md`).

pub mod error;
pub mod hexs;
pub mod kdf;
pub mod log;
pub mod object;
pub mod rng;
pub mod shamir;
pub mod time;
pub mod vault;
pub mod words;

pub use error::{Error, Result};
pub use kdf::KdfParams;
pub use object::Meta;
pub use vault::{words_from_pieces, Created, Header, Item, KeySlot, Vault, FORMAT, NEEDED, PIECES};
