//! Embeds a Windows version resource into guardiana.exe.
//!
//! Without it the file has no FileVersion, and Windows Installer then decides whether to replace it
//! by comparing timestamps instead of versions: for an unversioned file whose modified date is later
//! than its created date, it assumes a user edited it and keeps the old copy. That is why the
//! in-place MSI upgrade left the previous binary on disk on 14 Sep 2026 (docs/PRUEBAS.md).
//!
//! The resource is compiled with whatever resource compiler is on the machine. `zig rc` is the first
//! choice because the cross-build already needs zig (`cargo zigbuild --target x86_64-pc-windows-gnu`),
//! so this adds no new dependency and no new crate. If none is found the build still succeeds and
//! warns; `build/msi.ps1` is the gate that refuses to package an exe without a version.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // .git/HEAD only moves when the branch changes; .git/logs/HEAD gets a line on every commit,
    // which is what the fourth version component counts.
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/logs/HEAD");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let version = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".into());
    let mut parts: Vec<u16> = version
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();
    parts.resize(4, 0);
    // The fourth component counts commits. Two builds of the same release version but different
    // code must not look identical to Windows Installer: it refuses to overwrite a file "of an
    // equal version" and would silently keep the old binary (measured 15 Sep 2026). It must also
    // never go *down*, or the installer refuses just as firmly ("existing file is a higher
    // version"), so this is a count and not a hash. Same commit, same number: the build stays
    // reproducible (brief §10).
    parts[3] = commit_count();
    let comma = parts
        .iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(",");

    // ASCII only: a .rc file carries no encoding declaration, so accented text would depend on the
    // code page the resource compiler happens to assume.
    let rc = format!(
        r#"1 VERSIONINFO
FILEVERSION {comma}
PRODUCTVERSION {comma}
FILEOS 0x4
FILETYPE 0x1
{{
  BLOCK "StringFileInfo"
  {{
    BLOCK "000004b0"
    {{
      VALUE "CompanyName", "GUARDIANA"
      VALUE "FileDescription", "GUARDIANA - guardian DNS de la casa"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "guardiana"
      VALUE "LegalCopyright", "2026 GUARDIANA. GPL-3.0-or-later"
      VALUE "OriginalFilename", "guardiana.exe"
      VALUE "ProductName", "GUARDIANA"
      VALUE "ProductVersion", "{version}"
    }}
  }}
  BLOCK "VarFileInfo"
  {{
    VALUE "Translation", 0x0000, 0x04b0
  }}
}}
"#
    );

    // A build script has nowhere useful to panic to, and the workspace bans unwrap/expect: when the
    // resource cannot be written the build carries on and build/msi.ps1 catches the missing version.
    let Ok(out) = env::var("OUT_DIR") else {
        println!("cargo:warning=OUT_DIR unset: no version resource");
        return;
    };
    let out = PathBuf::from(out);
    let rc_path = out.join("guardiana.rc");
    let obj_path = out.join("guardiana-version.o");
    if let Err(e) = fs::write(&rc_path, rc) {
        println!("cargo:warning=could not write {}: {e}", rc_path.display());
        return;
    }

    match compile(&rc_path, &obj_path) {
        Some(_tool) => println!("cargo:rustc-link-arg-bins={}", obj_path.display()),
        None => println!(
            "cargo:warning=no resource compiler found (zig rc, llvm-rc or windres): \
             guardiana.exe will have no FileVersion and build/msi.ps1 will refuse to package it"
        ),
    }
}

/// How many commits lead to HEAD, capped at 65535; 0 when git is not available. Monotonic, so a
/// newer build always looks newer to Windows Installer.
fn commit_count() -> u16 {
    let Ok(out) = Command::new("git")
        .args(["rev-list", "--count", "HEAD"])
        .output()
    else {
        return 0;
    };
    if !out.status.success() {
        return 0;
    }
    let Ok(text) = String::from_utf8(out.stdout) else {
        return 0;
    };
    text.trim()
        .parse::<u32>()
        .map_or(0, |n| n.min(65_535) as u16)
}

/// Tries each known resource compiler and returns the name of the one that worked.
fn compile(rc: &Path, obj: &Path) -> Option<&'static str> {
    let zig = env::var("ZIG").ok().unwrap_or_else(|| "zig".into());
    let home_zig = env::var("HOME")
        .map(|h| format!("{h}/.local/zig/zig"))
        .unwrap_or_default();

    let zig_args = |z: &str| {
        (
            z.to_string(),
            vec![
                "rc".to_string(),
                "/:output-format".into(),
                "coff".into(),
                "/:target".into(),
                "x86_64".into(),
                "/fo".into(),
                obj.display().to_string(),
                rc.display().to_string(),
            ],
        )
    };

    let candidates: Vec<(String, Vec<String>)> = vec![
        zig_args(&zig),
        zig_args(&home_zig),
        (
            "llvm-rc".into(),
            vec![
                "/fo".into(),
                obj.display().to_string(),
                rc.display().to_string(),
            ],
        ),
        (
            "x86_64-w64-mingw32-windres".into(),
            vec![
                rc.display().to_string(),
                "-O".into(),
                "coff".into(),
                "-o".into(),
                obj.display().to_string(),
            ],
        ),
    ];

    for (program, args) in candidates {
        if program.is_empty() {
            continue;
        }
        if let Ok(status) = Command::new(&program).args(&args).status() {
            if status.success() && obj.exists() {
                return Some(if program.ends_with("windres") {
                    "windres"
                } else if program.ends_with("llvm-rc") {
                    "llvm-rc"
                } else {
                    "zig rc"
                });
            }
        }
    }
    None
}
