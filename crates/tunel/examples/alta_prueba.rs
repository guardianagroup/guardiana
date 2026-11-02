//! The whole VPN on Linux, for the end-to-end test (`.github/workflows/vpn.yml`, job «extremo»): a computer's side of
//! the sign-up with a real `guardiana-tunel-servidor`, reached at the address given (plain HTTP inside the test's own
//! network), and the configuration WireGuard takes, printed in its INI form with the secrets (test keys, made here and
//! thrown away with the test).
//!
//! `alta_prueba http://192.168.77.1:8793/vpn/alta`

use std::process::ExitCode;

use guardiana_tunel::alta;
use guardiana_tunel::conexion::pide_alta_a;
use guardiana_tunel::config::Config;
use guardiana_tunel::llaves::Llaves;

fn main() -> ExitCode {
    let Some(url) = std::env::args().nth(1) else {
        eprintln!("uso: alta_prueba http://servidor:puerto/vpn/alta");
        return ExitCode::from(2);
    };
    let llaves = match Llaves::nuevas() {
        Ok(l) => l,
        Err(e) => {
            eprintln!("alta_prueba: {e}");
            return ExitCode::FAILURE;
        }
    };
    let (oferta, pide) = alta::pide(&llaves);
    let respuesta = match pide_alta_a(&url, &pide) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("alta_prueba: {e}");
            return ExitCode::FAILURE;
        }
    };
    let alta = match alta::completa(&llaves, &oferta, &respuesta) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("alta_prueba: la respuesta no vale: {e}");
            return ExitCode::FAILURE;
        }
    };
    let config = Config::de(&llaves, &alta);
    eprintln!("alta_prueba: {}", config.resumen());
    print!("{}", config.a_ini().as_str());
    ExitCode::SUCCESS
}
