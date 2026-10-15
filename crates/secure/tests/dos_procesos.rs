//! Two operating-system processes exchange one verified message.
//!
//! The short-code numbers go to files. They are not carried by the post office.
//! The parent compares those files and only then tells each process, on stdin,
//! that a person compared them.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use guardiana_secure::puesto::Puesto;

#[test]
fn two_processes_exchange_one_verified_message() {
    let puesto = Puesto::listen().expect("post office");
    let dir = std::env::temp_dir().join(format!("guardiana-secure-{}", puesto.addr().port()));
    std::fs::create_dir_all(&dir).expect("dir");
    let alice_numbers = dir.join("alice.txt");
    let bob_numbers = dir.join("bob.txt");
    let result = dir.join("result.bin");
    let bin = std::env::var("CARGO_BIN_EXE_lado").expect("lado binary");

    let mut alice = Command::new(&bin)
        .args([
            "alice",
            &puesto.addr().to_string(),
            alice_numbers.to_str().expect("path"),
            result.to_str().expect("path"),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("alice");
    let mut bob = Command::new(&bin)
        .args([
            "bob",
            &puesto.addr().to_string(),
            bob_numbers.to_str().expect("path"),
            result.to_str().expect("path"),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("bob");

    let alice_text = wait_file(&alice_numbers);
    let bob_text = wait_file(&bob_numbers);
    assert_eq!(alice_text, bob_text, "the short codes did not match");
    alice
        .stdin
        .take()
        .expect("alice stdin")
        .write_all(b"1")
        .expect("tell alice");
    bob.stdin
        .take()
        .expect("bob stdin")
        .write_all(b"1")
        .expect("tell bob");

    let alice_status = alice.wait().expect("wait alice");
    let bob_status = bob.wait().expect("wait bob");
    assert!(alice_status.success(), "alice {alice_status}");
    assert!(bob_status.success(), "bob {bob_status}");

    let opened = std::fs::read(&result).expect("result");
    assert_eq!(opened, b"hola");
    let seen = puesto.seen();
    assert!(!seen.is_empty());
    assert!(!seen.windows(b"hola".len()).any(|window| window == b"hola"));
    let _ = std::fs::remove_dir_all(&dir);
}

fn wait_file(path: &std::path::Path) -> String {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(15) {
        if let Ok(text) = std::fs::read_to_string(path) {
            if !text.is_empty() {
                return text;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("the short code was not written to {}", path.display());
}
