//! GUARDIANA's VPN, the core that is the same on every system (in preparation; the owner, 1 Nov 2026, 16:21: «otra
//! sección aparte, que sea la VPN… del máximo nivel»; plan in docs/VPN.md).
//!
//! - [`llaves`]: the computer's WireGuard key pair (Curve25519).
//! - [`cuantico`]: the post-quantum half. WireGuard's handshake is Curve25519 only, and traffic recorded today could be
//!   opened by a quantum computer tomorrow; WireGuard lets a 32-byte preshared key be mixed into it. Here that key comes
//!   from ML-KEM-768 (FIPS 203), fresh for every sign-up, so an attacker must break both. Same idea as Rosenpass and as
//!   Mullvad's quantum-resistant tunnels.
//! - [`alta`]: the anonymous sign-up with a GUARDIANA server: the computer sends its public key and an ML-KEM key; the
//!   server answers with the tunnel's address, its own public key and the ML-KEM ciphertext. No account, no name.
//! - [`conexion`]: a whole connection, from fresh keys and the sign-up over HTTPS to the server's first handshake, with
//!   the system's half (opening the tunnel) passed in; if the server does not answer, the tunnel is closed again.
//! - [`config`]: the tunnel's configuration, ready for the Windows adapter (and printable for tests, without the
//!   private key).
//! - [`nt`]: that configuration as the bytes WireGuardNT's driver takes (`wireguard.h`'s structures).
//! - [`corte`]: the cut-off switch, as the plan of Windows Filtering Platform filters that keeps everything inside
//!   the tunnel (the same rules as WireGuard's own application for Windows).
//!
//! The Windows half (the WireGuardNT adapter and the firewall rules that keep everything inside the tunnel) lives next
//! to the program's firewall, in `guardiana-apps`.

pub mod alta;
pub mod conexion;
pub mod config;
pub mod corte;
pub mod cuantico;
pub mod llaves;
pub mod nt;

/// Why something in the tunnel's core could not be done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// No randomness was available from the system.
    SinAzar,
    /// A key, a ciphertext or a field from the server is not what it must be.
    Formato(&'static str),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SinAzar => f.write_str("el sistema no dio números al azar"),
            Self::Formato(que) => write!(f, "formato no válido: {que}"),
        }
    }
}

impl std::error::Error for Error {}

pub(crate) fn b64(b: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(b)
}

pub(crate) fn de_b64(s: &str) -> Option<Vec<u8>> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(s.trim())
        .ok()
}
