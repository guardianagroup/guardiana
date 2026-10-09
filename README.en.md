# Guardiana

*Languages: [Español](README.md) · **English** · [Português](README.pt.md)*

A program for Windows, Linux and macOS that turns the computer into the **DNS guardian of the home**:
first of itself, then of the phones, the TV and everything that uses the Wi‑Fi, without
installing anything on them. It sees which services each device tries to talk to, classifies it,
explains it in one sentence, writes it down in a hash-chained ledger and blocks only what the
user decides.

**Everything happens at home.** No account, no server of ours, zero telemetry. The only outgoing
connections are the ones the user triggers (activating the licence and the periodic subscription
check) and each one is recorded in the ledger itself.

- What it does not do, in those words: [docs/WHAT_IT_DOES_NOT_DO.md](docs/WHAT_IT_DOES_NOT_DO.md)
- Threat model: [docs/THREAT_MODEL.md](docs/THREAT_MODEL.md)
- How to check that what you installed is what was published: [docs/VERIFY.md](docs/VERIFY.md)
- Home Mode (phones without an app): [docs/HOGAR.md](docs/HOGAR.md) and the guides for the
  [router](docs/guias/router.md), [iPhone](docs/guias/iphone.md) and [Android](docs/guias/android.md)
- Which lists are used and under which licence: [docs/LISTS.md](docs/LISTS.md)
- Closed beta guide: [docs/BETA.md](docs/BETA.md)

The documentation is written in Spanish; the program's interface is in Spanish, English and
Portuguese.

## GUARDIANA ZERO, the browser

A browser of our own for Windows 10 and 11. It cuts what websites call behind your back (trackers,
advertising and registered data brokers) and shows every cut live: which company, from which country
and why. It removes tracking tags from addresses, keeps your marked data from going out to another
company, opens mandates for your AI with a signed receipt, and redacts your data before you ask it
anything. The first 7 days are free; after that it comes with the GUARDIANA subscription (the same key).

- Download, hash and signature: [guardianagroup.com/en/zero.html](https://guardianagroup.com/en/zero.html);
  in `ledger.jsonl`, its lines are the ones with version `zero-<version>`.
- Code: `crates/zero` (everything that decides, tested on Windows, Linux and macOS) and `zero/navegador`
  (the window, on WebView2). It is built and tested end to end in `.github/workflows/zero.yml`.

## Install

| System | Package | How |
|---|---|---|
| Windows 10/11 (64-bit) | `guardiana-<version>-windows-x64-en.msi` (Spanish: `…-x64.msi`; Portuguese: `…-x64-pt.msi`) | Double-click. Installs into Program Files and registers the service. |
| Debian, Ubuntu and derivatives | `guardiana_<version>_amd64.deb` | `sudo apt install ./guardiana_<version>_amd64.deb` |
| Other Linux with systemd | `guardiana-<version>-linux-x86_64.tar.gz` | Unpack and `sudo ./instalar.sh` |

On Linux and on the Mac, installing **does not change the system DNS**: that is done from the panel,
with consent, and undone in the same place. On Windows, a first install makes Guardiana the DNS (the
welcome screen says so before “Install”), and it is undone in the panel, under “Status”. Uninstalling turns Home Mode off, removes the firewall rule and restores
the DNS exactly as it was.

Before installing, compare the SHA‑256 fingerprint of the file with the one on the download page
and with the matching line of [`ledger.jsonl`](ledger.jsonl), the public ledger that is published before the
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

Nothing is blocked without the user's decision, always with a visible “unblock”: a specific name can be
blocked from the first minute, and wide blocks —by category, for the whole home, or the mode that blocks
everything not declared— wait until Guardiana has been watching that device for 24 hours. No signal is a verdict; the interface never says “malicious”.

## Build

Stable Rust, edition 2021, `Cargo.lock` in the repository.

```
cargo build --workspace --locked
cargo test --workspace --locked
```

Reproducible build in a digest-pinned container: `build/repro.sh` (see
[docs/VERIFY.md](docs/VERIFY.md)). Packages: `build/package.sh` (Linux) and `build/msi.ps1`
(Windows). Code layout and working rules: [docs/BRIEF.md](docs/BRIEF.md).

## Licence

GPL‑3.0‑or‑later. Third-party lists keep their own licence (EasyPrivacy: GPL‑3.0 / CC BY‑SA 3.0;
Peter Lowe's list: free use with attribution). Details in `docs/LISTS.md`. The typefaces the
panel serves from the machine itself (IBM Plex and Unbounded) travel with their OFL 1.1 licence
next to them, in [`crates/panel/static/fonts`](crates/panel/static/fonts/LEEME.md).

## Who makes it

GUARDIANA is an open-source project maintained by **Francisco Salvatierra Sánchez** (GUARDIANA
GROUP, Torre Empresarial PRODEGI, 18th floor, Bocagrande, Cartagena de Indias, Colombia), who signs
every release with the minisign key published in `build/pubkey/minisign.pub` and answers at
hola@guardianagroup.com. Website: https://guardianagroup.com.
