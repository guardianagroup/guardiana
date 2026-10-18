//! Windows half of «what Microsoft Defender is set to send to its cloud» (1.0.45).
//!
//! Defender keeps two documented settings about it, readable by any administrator and by the
//! system account the service runs as: `SubmitSamplesConsent` (whether files are sent as samples)
//! and `SpynetReporting` (the cloud protection membership, «MAPS»). A policy set by an
//! organisation lives under `Policies` and wins over the person's own choice. Read with
//! `reg.exe query`, one value at a time; nothing is written.

use std::os::windows::process::CommandExt;

use super::{lee_dword_de_reg, DefenderNube};

const PROPIA: &str = r"HKLM\SOFTWARE\Microsoft\Windows Defender\Spynet";
const DIRECTIVA: &str = r"HKLM\SOFTWARE\Policies\Microsoft\Windows Defender\Spynet";

fn valor(clave: &str, nombre: &str) -> Option<u32> {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let salida = std::process::Command::new("reg.exe")
        .args(["query", clave, "/v", nombre])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    if !salida.status.success() {
        return None;
    }
    lee_dword_de_reg(&String::from_utf8_lossy(&salida.stdout), nombre)
}

/// Policy first (it wins), then the person's own setting.
fn uno(nombre: &str) -> (Option<u32>, bool) {
    match valor(DIRECTIVA, nombre) {
        Some(v) => (Some(v), true),
        None => (valor(PROPIA, nombre), false),
    }
}

pub(crate) fn leer() -> DefenderNube {
    let (muestras, muestras_directiva) = uno("SubmitSamplesConsent");
    let (maps, maps_directiva) = uno("SpynetReporting");
    DefenderNube {
        muestras,
        muestras_directiva,
        maps,
        maps_directiva,
    }
}
