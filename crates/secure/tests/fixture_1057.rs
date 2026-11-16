//! Throwaway (15 Nov 2026): prints an identity locked by the vodozemac of the 1.0.57, to keep it as a fixture for the
//! 1.0.58 (vodozemac 0.11.2). This branch is never merged.

#[test]
fn imprime_identidad_1057() {
    let clave = [7u8; 32];
    let i = guardiana_secure::Identity::new();
    let bloque = i.lock(&clave);
    panic!(
        "FIXTURE-1057 bloque={bloque} curve={} ed={}",
        i.public_key().to_base64(),
        i.signing_key().to_base64()
    );
}
