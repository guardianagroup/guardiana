//! Two processes use the direct connection, the same path two computers use.
//! The parent is the out-of-band comparison. Neither process writes the plaintext.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn two_processes_use_the_direct_connection() {
    let dir = std::env::temp_dir().join(format!("guardiana-secure-equipo-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let alice_numbers = dir.join("alice.txt");
    let bob_numbers = dir.join("bob.txt");
    let bin = std::env::var("CARGO_BIN_EXE_equipo").unwrap();
    let bind = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));

    let mut bob = Command::new(&bin)
        .args(["bob", &bind.to_string(), bob_numbers.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let port = read_port(bob.stdout.as_mut().unwrap());
    let alice_addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let mut alice = Command::new(&bin)
        .args([
            "alice",
            &alice_addr.to_string(),
            alice_numbers.to_str().unwrap(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();

    let alice_text = wait_file(&alice_numbers);
    let bob_text = wait_file(&bob_numbers);
    assert_eq!(alice_text, bob_text);
    alice.stdin.take().unwrap().write_all(b"1").unwrap();
    bob.stdin.take().unwrap().write_all(b"1").unwrap();

    let alice_status = alice.wait().unwrap();
    let bob_status = bob.wait().unwrap();
    assert!(alice_status.success(), "{alice_status}");
    assert!(bob_status.success(), "{bob_status}");
    let _ = std::fs::remove_dir_all(&dir);
}

fn read_port(stdout: &mut impl std::io::Read) -> u16 {
    let mut line = String::new();
    BufReader::new(stdout).read_line(&mut line).unwrap();
    let port = line.trim().strip_prefix("PORT ").unwrap();
    port.parse().unwrap()
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
    panic!("missing {}", path.display());
}
