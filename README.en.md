# Guardiana

*Versión en español: [README.md](README.md).*

> **In 30 seconds.** You let an AI talk to the network: GUARDIANA names the services it called —
> never the content — and lets you cut the line. For the first 24 hours it only watches. Free, open
> source (GPL‑3.0), Windows and Linux. **Version 1.0: 5 October 2026, 15:00 UTC.** There is no
> download here until that day: there is the code it is built from, and the hash of every release is
> published before the file itself.
>
> To try it from source, without touching the machine's DNS or asking for administrator rights:
>
> ```bash
> cargo build --locked
> GUARDIANA_DATA=/tmp/guardiana ./target/debug/guardiana observe --listen 127.0.0.1:5335 --upstream 1.1.1.1
> dig -p 5335 @127.0.0.1 example.com     # in another terminal
> GUARDIANA_DATA=/tmp/guardiana ./target/debug/guardiana ledger
> ```
>
> That starts the guardian on an unprivileged port, resolves one query and prints what it wrote down.


A program for Windows and Linux that turns the computer into the **DNS guardian of the home**:
first of itself, then of the phones, the TV and everything that uses the Wi‑Fi, without
installing anything on them. It sees which services each device tries to talk to, classifies it,
explains it in one sentence, writes it down in a hash-chained ledger and blocks only what the
user decides.

**Everything happens at home.** No account, no server of ours, zero telemetry. The only outgoing
connections are the ones the user triggers (activate a licence, check for a new version, update
the lists) and each one is recorded in the ledger itself.

- What it does not do, in those words: [docs/WHAT_IT_DOES_NOT_DO.md](docs/WHAT_IT_DOES_NOT_DO.md)
- Threat model: [docs/THREAT_MODEL.md](docs/THREAT_MODEL.md)
- How to check that what you installed is what was published: [docs/VERIFY.md](docs/VERIFY.md)
- Home Mode (phones without an app): [docs/HOGAR.md](docs/HOGAR.md) and the guides for the
  [router](docs/guias/router.md), [iPhone](docs/guias/iphone.md) and [Android](docs/guias/android.md)
- Which lists are used and under which licence: [docs/LISTS.md](docs/LISTS.md)
- Closed beta guide: [docs/BETA.md](docs/BETA.md)
- Decisions taken and the test log live in the working repository; everything that affects what the
  program promises is here (above) and at https://guardianagroup.com

The documentation is written in Spanish; the program's interface is in Spanish, with English on
the way.

## Install

| System | Package | How |
|---|---|---|
| Windows 10/11 (64-bit) | `guardiana-<version>-windows-x64.msi` | Double-click. Installs into Program Files and registers the service. |
| Debian, Ubuntu and derivatives | `guardiana_<version>_amd64.deb` | `sudo apt install ./guardiana_<version>_amd64.deb` |
| Other Linux with systemd | `guardiana-<version>-linux-x86_64.tar.gz` | Unpack and `sudo ./instalar.sh` |

Installing **does not change the system DNS**: that is done from the panel, with consent, and
undone in the same place. Uninstalling turns Home Mode off, removes the firewall rule and restores
the DNS exactly as it was.

Before installing, compare the SHA‑256 fingerprint of the file with `SHA256SUMS` and with the
matching line of [`ledger.jsonl`](ledger.jsonl), the public record that is published before the
download. After installing, `guardiana verify` checks it on your machine.

## Use

```
guardiana panel          opens the panel in the browser (the only interface)
guardiana verify         fingerprint, signature, service, system DNS, ports, lists, chain
guardiana dns --status   where the system DNS points
guardiana hogar status   Home Mode status
guardiana ledger --check checks the ledger chain
guardiana export         exports the ledger as CSV or JSON
```

Nothing is blocked without the user's decision, never before 24 hours observing a device, always
with a visible "undo". No signal is a verdict; the interface never says "malicious".

## Build

Stable Rust, edition 2021, `Cargo.lock` in the repository.

```
cargo build --workspace --locked
cargo test --workspace --locked
```

Reproducible build in a digest-pinned container: `build/repro.sh` (see
[docs/VERIFY.md](docs/VERIFY.md)). Packages: `build/package.sh` (Linux) and `build/msi.ps1`
(Windows). Code layout and working rules: [CLAUDE.md](CLAUDE.md) and
[docs/BRIEF.md](docs/BRIEF.md).

## Licence

GPL‑3.0‑or‑later. Third-party lists keep their own licence (EasyPrivacy: GPL‑3.0 / CC BY‑SA 3.0;
Peter Lowe's list: free use with attribution). Details in `docs/LISTS.md`.
