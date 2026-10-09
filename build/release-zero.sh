#!/usr/bin/env bash
# Publish GUARDIANA ZERO on its own, when GUARDIANA does not change (9 Oct 2026: the browser moved
# to the subscription while GUARDIANA 1.0.8 stayed as it was). The same order as release.sh, for one
# file:
#   1. the browser built and driven end to end by zero.yml (the artifact's folder, with its
#      SHA256SUMS), checked here before anything is signed
#   2. minisign signature with the release key (never in CI)
#   3. its line in ledger.jsonl, as version "zero-<version>" so it never passes for a GUARDIANA
#      version (build/cliente/descarga.py skips these lines when it looks for the latest one)
#   4. the line signed and uploaded to Rekor; its uuid goes into the line
#   5. commit ledger.jsonl — only then may the download be published
#
#     build/release-zero.sh <folder with guardiana-zero-<version>-windows-x64.exe and SHA256SUMS>
#
# Environment: GUARDIANA_KEYS (directory with minisign.key, default $HOME/guardiana-claves).
set -euo pipefail
if [ -z "${PYTHON:-}" ]; then
    if python3 -c 'import sys' >/dev/null 2>&1; then PYTHON=python3; else PYTHON=python; fi
fi
if [ -n "${CI:-}" ] || [ -n "${GITHUB_ACTIONS:-}" ]; then
    echo "release-zero.sh: never run in CI: the private key must not be there (brief §10)." >&2
    exit 1
fi

cd "$(dirname "$0")/.."
carpeta="${1:?release-zero.sh needs the folder of the zero.yml artifact}"
keys="${GUARDIANA_KEYS:-$HOME/guardiana-claves}"
commit="$(git rev-parse HEAD)"
date="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

[ -f "$keys/minisign.key" ] || { echo "release-zero.sh: no $keys/minisign.key" >&2; exit 1; }
if command -v minisign >/dev/null 2>&1; then
    firmar() { minisign -Sm "$1" -s "$keys/minisign.key" -t "$2"; }
elif command -v rsign >/dev/null 2>&1; then
    firmar() { rsign sign -s "$keys/minisign.key" -x "$1.minisig" -t "$2" "$1"; }
else
    echo "release-zero.sh: minisign or rsign is required (cargo install rsign2)" >&2
    exit 1
fi
if grep -q DEV-NOT-A-KEY build/pubkey/minisign.pub; then
    echo "release-zero.sh: build/pubkey/minisign.pub is the development placeholder" >&2
    exit 1
fi
if [ -n "$(git status --porcelain)" ]; then
    echo "release-zero.sh: the working tree must be clean" >&2
    exit 1
fi

# 1. The tested file, and only that one.
version="$(sed -n 's/^version = "\(.*\)"/\1/p' zero/navegador/Cargo.toml | head -n 1)"
nombre="guardiana-zero-$version-windows-x64.exe"
[ -f "$carpeta/$nombre" ] || { echo "release-zero.sh: $carpeta has no $nombre (zero/navegador is $version)" >&2; exit 1; }
[ -f "$carpeta/SHA256SUMS" ] || { echo "release-zero.sh: $carpeta has no SHA256SUMS" >&2; exit 1; }
(cd "$carpeta" && grep -F "  $nombre" SHA256SUMS | sha256sum -c -) || {
    echo "release-zero.sh: $nombre does not match the SHA256SUMS that came with it; nothing is published." >&2
    exit 1
}
etiqueta="zero-$version"
if grep -q "\"version\":\"$etiqueta\"" ledger.jsonl; then
    echo "release-zero.sh: $etiqueta is already in ledger.jsonl" >&2
    exit 1
fi
dist="dist/$etiqueta"
out="build/out"
mkdir -p "$dist" "$out"
cp "$carpeta/$nombre" "$dist/$nombre"

# 2. The signature.
firmar "$dist/$nombre" "guardiana $etiqueta $nombre"
sha="$(sha256sum "$dist/$nombre" | cut -d' ' -f1)"
sig="$(sed -n '2p' "$dist/$nombre.minisig")"

# 3. The line.
line="$("$PYTHON" -c '
import json, sys
v, c, d, n, sha, sig = sys.argv[1:7]
print(json.dumps({"version": v, "commit": c, "date": d, "files": [{"name": n, "sha256_unsigned": sha,
      "sha256_signed": sha, "minisign": sig, "reproducible": False}], "rekor_uuid": ""},
      separators=(",", ":")))
' "$etiqueta" "$commit" "$date" "$nombre" "$sha" "$sig")"

# 4. Rekor, over its REST API, exactly as release.sh does it.
echo "$line" > "$out/ledger-line.json"
firmar "$out/ledger-line.json" "guardiana $etiqueta ledger line"
rekor_uuid="$("$PYTHON" - "$out/ledger-line.json" "$out/ledger-line.json.minisig" build/pubkey/minisign.pub <<'PYREKOR'
import base64, json, sys, urllib.request, urllib.error
art, sig, pub = (open(p, "rb").read() for p in sys.argv[1:4])
cuerpo = {"apiVersion": "0.0.1", "kind": "rekord", "spec": {
    "data": {"content": base64.b64encode(art).decode()},
    "signature": {"format": "minisign", "content": base64.b64encode(sig).decode(),
                  "publicKey": {"content": base64.b64encode(pub).decode()}}}}
req = urllib.request.Request("https://rekor.sigstore.dev/api/v1/log/entries",
                             data=json.dumps(cuerpo).encode(),
                             headers={"Content-Type": "application/json"})
try:
    with urllib.request.urlopen(req, timeout=30) as r:
        print(r.headers.get("Location", "").rsplit("/", 1)[-1])
except urllib.error.HTTPError as e:
    print("", file=sys.stdout)
    print("release-zero.sh: Rekor refused the entry: %s %s" % (e.code, e.read()[:200].decode("utf-8", "replace")), file=sys.stderr)
PYREKOR
)"
[ -n "$rekor_uuid" ] || echo "release-zero.sh: no rekor uuid; upload $out/ledger-line.json by hand before publishing." >&2
line="$("$PYTHON" -c 'import json,sys; l=json.loads(sys.argv[1]); l["rekor_uuid"]=sys.argv[2]; print(json.dumps(l, separators=(",",":")))' "$line" "$rekor_uuid")"

# 5. Append and commit. The download is published only after this commit is pushed.
echo "$line" >> ledger.jsonl
git add ledger.jsonl
git commit -m "Publish $etiqueta to ledger.jsonl (rekor ${rekor_uuid:-pending})"
echo "== ledger line:"
echo "$line"
echo "== now push, then upload $dist to the web. Never before."
