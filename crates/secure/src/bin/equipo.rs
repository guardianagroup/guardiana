//! One computer in a two-computer SECURE hello.
//!
//! The three numbers are written to a file, not decided from the socket.
//! Stdin must be `1` before this process verifies the other device. The opened
//! text is compared in memory and is not written out.

use std::io::{Read, Write};
use std::net::SocketAddr;
use std::path::Path;

use guardiana_secure::enlace::{Canal, Escucha};
use guardiana_secure::verificar::{empaquetar, Inicio, Pendiente, Trust};
use guardiana_secure::{Envelope, Identity};

fn main() {
    if run().is_err() {
        std::process::exit(1);
    }
}

fn run() -> Result<(), ()> {
    let mut args = std::env::args().skip(1);
    let role = args.next().ok_or(())?;
    let addr: SocketAddr = args.next().ok_or(())?.parse().map_err(|_| ())?;
    let decimals = args.next().ok_or(())?;
    match role.as_str() {
        "bob" => bob(addr, Path::new(&decimals)),
        "alice" => alice(addr, Path::new(&decimals)),
        _ => Err(()),
    }
}

fn bob(addr: SocketAddr, decimals: &Path) -> Result<(), ()> {
    let escucha = Escucha::bind(addr).map_err(|_| ())?;
    let bound = escucha.addr().map_err(|_| ())?;
    println!("PORT {}", bound.port());
    let _ = std::io::stdout().flush();
    let mut canal = escucha.uno().map_err(|_| ())?;
    let mut identity = Identity::new();
    let one_time = identity.fresh_one_time_key().map_err(|_| ())?;
    let start = Inicio::new();
    let alice_sas = canal.claves(start.public_key(), false).map_err(|_| ())?;
    let code = start.establish(alice_sas).map_err(|_| ())?;
    write_decimals(decimals, code.decimals())?;
    if !person_compared() {
        return Err(());
    }
    let mine = empaquetar(&identity, &code).map_err(|_| ())?;
    let theirs = canal.pruebas(&mine, false).map_err(|_| ())?;
    let pending = Pendiente::sostener(code, &theirs).map_err(|_| ())?;
    let verified = pending.la_persona_dice(true).map_err(|_| ())?;
    let mut trust = Trust::new();
    trust.aceptar(verified);
    canal.enviar(one_time.as_bytes()).map_err(|_| ())?;
    let envelope = decode_envelope(&canal.recibir().map_err(|_| ())?)?;
    let (_, opened) = identity
        .receive(&trust, verified.curve(), &envelope)
        .map_err(|_| ())?;
    if opened != b"hola" {
        return Err(());
    }
    Ok(())
}

fn alice(addr: SocketAddr, decimals: &Path) -> Result<(), ()> {
    let mut canal = Canal::conectar(addr).map_err(|_| ())?;
    let identity = Identity::new();
    let start = Inicio::new();
    let bob_sas = canal.claves(start.public_key(), true).map_err(|_| ())?;
    let code = start.establish(bob_sas).map_err(|_| ())?;
    write_decimals(decimals, code.decimals())?;
    if !person_compared() {
        return Err(());
    }
    let mine = empaquetar(&identity, &code).map_err(|_| ())?;
    let theirs = canal.pruebas(&mine, true).map_err(|_| ())?;
    let pending = Pendiente::sostener(code, &theirs).map_err(|_| ())?;
    let verified = pending.la_persona_dice(true).map_err(|_| ())?;
    let mut trust = Trust::new();
    trust.aceptar(verified);
    let one_time = read_key(&canal.recibir().map_err(|_| ())?)?;
    let mut conversation = identity.start(verified.curve(), one_time).map_err(|_| ())?;
    let envelope = trust.send(&mut conversation, b"hola").map_err(|_| ())?;
    canal.enviar(&encode_envelope(&envelope)?).map_err(|_| ())?;
    Ok(())
}

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

fn read_key(bytes: &[u8]) -> Result<vodozemac::Curve25519PublicKey, ()> {
    vodozemac::Curve25519PublicKey::from_slice(bytes).map_err(|_| ())
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
