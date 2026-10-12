# GUARDIANA — How to verify that what you have is what we published

*(Spanish original: [VERIFY.md](VERIFY.md).)*

GUARDIANA is published so that anyone can check, without trusting us, that the installed program
is exactly the one that was published and that it was built from the public code.

There are three independent checks, from cheapest to strongest:

| Check | What it proves | Linux | Windows | macOS |
|---|---|---|---|---|
| **SHA‑256** against `ledger.jsonl` | the file is the one listed in this repo | ✔ | ✔ | ✔ |
| **minisign** signature | the file was signed by the GUARDIANA release key, which never touches CI | ✔ | ✔ | ✔ |
| **Rekor** (Sigstore transparency log) | the ledger line existed on that date and was not rewritten | ✔ | ✔ | ✔ |
| OS code signature | the OS vendor's chain of trust | — (none on Linux) | ✘ not yet: no Authenticode certificate (see below) | ✘ not notarized (see below) |
| Reproducible build | the Linux binary comes bit‑for‑bit from the public commit | ✔ `build/repro.sh` | — | — |

## The release key

minisign public key (also in `build/pubkey/minisign.pub` and on guardianagroup.com):

```
RWS+xz+N6DvnuDlJCAIOUEku8PLZ9pP1cUaoQnusfduLWpVIVmPXM16r
```

Key ID `B8E73BE88D3FC7BE`. The private key lives only on the release machine; GitHub Actions
builds the packages but never signs them. If this key ever changed, treat it as a serious warning:
the key is the program's identity.

## One minute: `guardiana verify`

On the machine where it is installed:

```bash
guardiana verify          # add --json for machine‑readable output
```

It prints the SHA‑256 of the installed executable, checks the `.minisig` next to it against the
public key embedded in the program, looks the hash up in `ledger.jsonl` and tells you which version
and date it belongs to, and then reports the service, system DNS, open ports (with Home Mode off:
**none**), list dates and the integrity of the local event log. The dashboard shows the same at
`/verify`.

## By hand, without GUARDIANA

### 1. Hash

```bash
# Linux / macOS
sha256sum guardiana-1.0.15-linux-x86_64             # or: shasum -a 256 <file>
# Windows (PowerShell)
Get-FileHash .\guardiana-1.0.15-windows-x64.msi -Algorithm SHA256
```

Compare with the line for that version in [`ledger.jsonl`](../ledger.jsonl):

- `sha256_unsigned`: hash of the binary as it comes out of the build.
- `sha256_signed`: hash of the binary after an operating-system code signature is added (it
  lives inside the file, so the hash would change). Today no file carries one, so both values are
  equal for every file, Windows included.

### 2. minisign

```bash
minisign -Vm guardiana-1.0.15-linux-x86_64 -P RWS+xz+N6DvnuDlJCAIOUEku8PLZ9pP1cUaoQnusfduLWpVIVmPXM16r
```

`rsign` (`cargo install rsign2`) reads the same format: `rsign verify -P <key> <file>`.
The `.minisig` is downloaded next to each file and covers the file exactly as you download it, on
every platform.

### 3. Windows — honest note

The Windows installers are **not Authenticode-signed yet**: a code-signing certificate is a yearly
cost we have not paid so far. Windows SmartScreen will therefore say *Unknown publisher* and may
show "Windows protected your PC" the first time (*More info → Run anyway*). We never ask you to
turn off your antivirus. What stands behind the file instead is the same as on Linux and macOS:
its SHA‑256 and minisign signature, published in this repo and in Rekor before the download
exists. If we add a certificate, this section and the ledger (`sha256_signed`) will say so.

### 4. macOS — honest note

The `.app` is **not notarized** and not signed with an Apple Developer ID. Apple's developer
program is a yearly paid subscription and we decided not to pay it yet; Gatekeeper will therefore
warn on first launch (right‑click → Open, or *System Settings → Privacy & Security → Open anyway*).
What replaces notarization is the same as on Linux: the SHA‑256 and the minisign signature,
published in this repo before the download exists. If that is not enough for you, we understand —
build it yourself from the public source, or wait until we add notarization (it is on the list).

### 5. Rekor

Every ledger line is uploaded to Rekor (Sigstore's public transparency log) **before** the download
goes live. The uploaded data is the line with `"rekor_uuid":""` (an entry cannot contain its own
ID), serialized without spaces and with a trailing newline:

```bash
grep '"version":"1.0.15"' ledger.jsonl | python3 -c 'import json,sys; l=json.loads(sys.stdin.read()); u=l["rekor_uuid"]; l["rekor_uuid"]=""; open("line.json","w").write(json.dumps(l,separators=(",",":"))+"\n"); print(u)'
sha256sum line.json
curl -s https://rekor.sigstore.dev/api/v1/log/entries/<uuid> | python3 -c 'import json,sys,base64; e=list(json.load(sys.stdin).values())[0]; print(json.loads(base64.b64decode(e["body"]))["spec"]["data"]["hash"]["value"])'
```

Both hashes must match. The entry also carries the minisign signature of that line and the public
key, so the log proves *who* published it and *when*, independently of this repository or our
website.

## Reproducing the build (Linux)

`build/repro.sh` compiles GUARDIANA in a container pinned by digest (same compiler, same
dependencies, build date taken from the commit) and prints the SHA‑256 of the binaries. Anyone
running it on the same commit must obtain the `sha256_unsigned` values from `ledger.jsonl`.
Installers and the ZERO browser are built on GitHub's runners and are checked by hash only
(`"reproducible": false` in the ledger). Independent reproductions get a row here:

| Date | Who | Version | Result |
|---|---|---|---|
| — | — | — | nobody yet — you could be first |

## What this does *not* prove

- That the code is free of bugs. It proves that what you run is what we published, nothing more.
- That a signing key can never be stolen. The minisign key is kept offline, apart from CI; if a
  file ever appeared that is not in the ledger and not in Rekor, it is not ours.
