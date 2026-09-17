//! What the product writes, the independent reader opens — with password, with the 24 words, and
//! it notices a broken log line.
#![allow(clippy::panic, clippy::unwrap_used)]

use guardiana_vault::{KdfParams, Vault};
use guardiana_vault_reader as lector;

const FAST: KdfParams = KdfParams {
    memoria_kib: 8 * 1024,
    pasadas: 1,
    hilos: 1,
};

#[test]
fn reader_opens_what_the_product_wrote() {
    let dir = std::env::temp_dir().join(format!("gdn-compat-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let created =
        Vault::create(&dir, b"una clave larga", FAST, "prueba").unwrap_or_else(|e| panic!("{e}"));
    let words = created.words.clone();
    let big: Vec<u8> = (0..(1 << 20) + 77).map(|i| (i % 253) as u8).collect();
    created
        .vault
        .add(
            "grande.bin",
            &mut big.as_slice(),
            big.len() as u64,
            "prueba",
        )
        .unwrap_or_else(|e| panic!("{e}"));
    created
        .vault
        .add("nota.txt", &mut (&b"hola"[..]), 4, "prueba")
        .unwrap_or_else(|e| panic!("{e}"));
    drop(created.vault);

    let h = lector::header(&dir).unwrap_or_else(|e| panic!("{e}"));
    assert!(matches!(
        lector::master_key(&h, "contrasena", b"otra"),
        Err(lector::Error::WrongKey)
    ));
    let k =
        lector::master_key(&h, "contrasena", b"una clave larga").unwrap_or_else(|e| panic!("{e}"));
    let items = lector::list(&dir, &k).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(items.len(), 2);
    let mut out = Vec::new();
    let it = lector::extract(&dir, &k, "grande.bin", &mut out).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(out, big);
    assert_eq!(it.tamano, big.len() as u64);

    let e = lector::entropy_from_words(&words).unwrap_or_else(|e| panic!("{e}"));
    let k2 = lector::master_key(&h, "palabras", &e).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(k, k2);

    let (lines, check) = lector::log(&dir, &h.id).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(lines.len(), 3);
    assert_eq!(check, Ok(3));
    let p = dir.join("registro.jsonl");
    let text = std::fs::read_to_string(&p)
        .unwrap_or_default()
        .replace("\"accion\":\"guardar\"", "\"accion\":\"leer\"");
    std::fs::write(&p, text).unwrap_or_default();
    let (_, check) = lector::log(&dir, &h.id).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(check, Err(2));
    let _ = std::fs::remove_dir_all(&dir);
}
