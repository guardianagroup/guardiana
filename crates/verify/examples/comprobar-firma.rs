//! Check a published file against the key that is baked into the program.
//!
//! The signatures are made with `rsign`; checking them with `rsign` again only proves the tool
//! agrees with itself. This uses the other implementation -- `minisign-verify`, the one the
//! program uses on the user's machine -- and the public key compiled into the binary, which is
//! what the person actually trusts. Written for the 1.0.0 signing rehearsal, 21 September 2026.
//!
//!     cargo run -p guardiana-verify --example comprobar-firma -- dist/1.0.0/guardiana-1.0.0-linux-x86_64
use std::path::PathBuf;

fn main() {
    let mut malas = 0;
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("uso: comprobar-firma <archivo>...");
        std::process::exit(2);
    }
    for arg in &args {
        let archivo = PathBuf::from(arg);
        let firma = PathBuf::from(format!("{arg}.minisig"));
        let nombre = archivo
            .file_name()
            .map_or_else(|| arg.clone(), |n| n.to_string_lossy().into_owned());
        let resultado = (|| -> Result<(), String> {
            let texto = std::fs::read_to_string(&firma).map_err(|e| format!("sin firma: {e}"))?;
            let pk = minisign_verify::PublicKey::decode(guardiana_core::identity::PUBLIC_KEY_TEXT)
                .map_err(|e| format!("clave pública ilegible: {e}"))?;
            let sig = minisign_verify::Signature::decode(&texto)
                .map_err(|e| format!("firma ilegible: {e}"))?;
            let bytes = std::fs::read(&archivo).map_err(|e| format!("archivo ilegible: {e}"))?;
            pk.verify(&bytes, &sig, false)
                .map_err(|e| format!("NO comprueba: {e}"))
        })();
        match resultado {
            Ok(()) => println!("OK   {nombre}"),
            Err(e) => {
                println!("MAL  {nombre}: {e}");
                malas += 1;
            }
        }
    }
    if malas > 0 {
        std::process::exit(1);
    }
}
