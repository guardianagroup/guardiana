//! One side of a two-process SECURE hello.
//!
//! The three short-code numbers are written to a file and are not sent through
//! the post office. This process confirms the other device only after stdin
//! says the numbers were compared on that separate channel.

use std::io::Read;
use std::net::SocketAddr;
use std::path::Path;

use guardiana_secure::puesto;
use guardiana_secure::verificar::{Codigo, Inicio, Trust};
use guardiana_secure::{Envelope, Identity};
use vodozemac::sas::Mac;
use vodozemac::{Curve25519PublicKey, Ed25519PublicKey, Ed25519Signature};

fn main() {
    if run().is_err() {
        std::process::exit(1);
    }
}

fn run() -> Result<(), ()> {
    let mut args = std::env::args().skip(1);
    let role = args.next().ok_or(())?;
    let addr: SocketAddr = args.next().ok_or(())?.parse().map_err(|_| ())?;
    let decimals_path = args.next().ok_or(())?;
    let result_path = args.next().ok_or(())?;
    match role.as_str() {
        "alice" => alice(addr, Path::new(&decimals_path)),
        "bob" => bob(addr, Path::new(&decimals_path), Path::new(&result_path)),
        _ => Err(()),
    }
}

fn alice(addr: SocketAddr, decimals_path: &Path) -> Result<(), ()> {
    let identity = Identity::new();
    let start = Inicio::new();
    puesto::put(addr, "a-sas", start.public_key().as_bytes()).map_err(|_| ())?;
    let bob_sas = read_key(&puesto::get(addr, "b-sas").map_err(|_| ())?)?;
    let code = start.establish(bob_sas).map_err(|_| ())?;
    write_decimals(decimals_path, code.decimals())?;
    if !person_compared() {
        return Err(());
    }
    puesto::put(addr, "a-proof", &proof(&identity, &code)?).map_err(|_| ())?;
    let (bob_curve, bob_signing, bob_mac, bob_signature) =
        read_proof(&puesto::get(addr, "b-proof").map_err(|_| ())?)?;
    let verified = code
        .confirm(bob_curve, bob_signing, &bob_mac, &bob_signature, true)
        .map_err(|_| ())?;
    let one_time = read_key(&puesto::get(addr, "b-otk").map_err(|_| ())?)?;
    let mut trust = Trust::new();
    trust.insert(verified);
    let mut conversation = identity.start(verified.curve(), one_time).map_err(|_| ())?;
    let envelope = trust.send(&mut conversation, b"hola").map_err(|_| ())?;
    puesto::put(addr, "msg", &encode_envelope(&envelope)?).map_err(|_| ())?;
    Ok(())
}

fn bob(addr: SocketAddr, decimals_path: &Path, result_path: &Path) -> Result<(), ()> {
    let mut identity = Identity::new();
    let one_time = identity.fresh_one_time_key().map_err(|_| ())?;
    let start = Inicio::new();
    puesto::put(addr, "b-sas", start.public_key().as_bytes()).map_err(|_| ())?;
    let alice_sas = read_key(&puesto::get(addr, "a-sas").map_err(|_| ())?)?;
    let code = start.establish(alice_sas).map_err(|_| ())?;
    write_decimals(decimals_path, code.decimals())?;
    if !person_compared() {
        return Err(());
    }
    puesto::put(addr, "b-otk", one_time.as_bytes()).map_err(|_| ())?;
    puesto::put(addr, "b-proof", &proof(&identity, &code)?).map_err(|_| ())?;
    let (alice_curve, alice_signing, alice_mac, alice_signature) =
        read_proof(&puesto::get(addr, "a-proof").map_err(|_| ())?)?;
    let verified = code
        .confirm(
            alice_curve,
            alice_signing,
            &alice_mac,
            &alice_signature,
            true,
        )
        .map_err(|_| ())?;
    let mut trust = Trust::new();
    trust.insert(verified);
    let envelope = decode_envelope(&puesto::get(addr, "msg").map_err(|_| ())?)?;
    let (_, opened) = identity
        .receive(&trust, verified.curve(), &envelope)
        .map_err(|_| ())?;
    std::fs::write(result_path, opened).map_err(|_| ())?;
    Ok(())
}

/// The numbers were compared outside this socket. A missing or different byte
/// means they were not, and this process stops before it confirms anyone.
fn person_compared() -> bool {
    let mut byte = [0u8; 1];
    std::io::stdin().read_exact(&mut byte).is_ok() && byte[0] == b'1'
}

fn write_decimals(path: &Path, decimals: (u16, u16, u16)) -> Result<(), ()> {
    std::fs::write(
        path,
        format!("{} {} {}", decimals.0, decimals.1, decimals.2),
    )
    .map_err(|_| ())
}

fn proof(device: &Identity, code: &Codigo) -> Result<Vec<u8>, ()> {
    let mac = code.mac(device.signing_key(), device.public_key());
    if mac.as_bytes().len() != 32 {
        return Err(());
    }
    let signature = device.sign(&code.transcript(device.signing_key(), device.public_key()));
    let mut out = Vec::with_capacity(160);
    out.extend_from_slice(device.public_key().as_bytes());
    out.extend_from_slice(device.signing_key().as_bytes());
    out.extend_from_slice(mac.as_bytes());
    out.extend_from_slice(&signature.to_bytes());
    Ok(out)
}

fn read_key(bytes: &[u8]) -> Result<Curve25519PublicKey, ()> {
    Curve25519PublicKey::from_slice(bytes).map_err(|_| ())
}

fn read_proof(
    bytes: &[u8],
) -> Result<(Curve25519PublicKey, Ed25519PublicKey, Mac, Ed25519Signature), ()> {
    if bytes.len() != 160 {
        return Err(());
    }
    let curve = Curve25519PublicKey::from_slice(&bytes[..32]).map_err(|_| ())?;
    let mut signing_bytes = [0u8; 32];
    signing_bytes.copy_from_slice(&bytes[32..64]);
    let signing = Ed25519PublicKey::from_slice(&signing_bytes).map_err(|_| ())?;
    let mac = Mac::from_slice(&bytes[64..96]);
    let signature = Ed25519Signature::from_slice(&bytes[96..]).map_err(|_| ())?;
    Ok((curve, signing, mac, signature))
}

fn encode_envelope(envelope: &Envelope) -> Result<Vec<u8>, ()> {
    let (kind, bytes) = envelope.to_wire();
    let kind = u32::try_from(kind).map_err(|_| ())?;
    let mut out = Vec::with_capacity(4 + bytes.len());
    out.extend_from_slice(&kind.to_be_bytes());
    out.extend_from_slice(&bytes);
    Ok(out)
}

fn decode_envelope(bytes: &[u8]) -> Result<Envelope, ()> {
    if bytes.len() < 4 {
        return Err(());
    }
    let mut kind_bytes = [0u8; 4];
    kind_bytes.copy_from_slice(&bytes[..4]);
    Envelope::from_wire(u32::from_be_bytes(kind_bytes) as usize, &bytes[4..]).map_err(|_| ())
}
