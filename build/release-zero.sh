#!/usr/bin/env bash
# Publish GUARDIANA ZERO on its own, when GUARDIANA does not change (9 Oct 2026: the browser moved
# to the subscription while GUARDIANA 1.0.8 stayed as it was). The same order as release.sh, for
# the browser's files (the .exe and, since 1.0.7, its three installers, one per language):
#   1. the files built and driven end to end by zero.yml (the artifact's folder, with its
#      SHA256SUMS), checked here before anything is signed
#   2. minisign signature of each with the release key (never in CI)
#   3. one line in ledger.jsonl, as version "zero-<version>" so it never passes for a GUARDIANA
#      version (build/cliente/descarga.py skips these lines when it looks for the latest one)
#   4. the line signed and uploaded to Rekor; its uuid goes into the line
#   5. commit ledger.jsonl — only then may the download be published
#
#     build/release-zero.sh <folder with guardiana-zero-<version>-windows-x64.exe, its .msi and SHA256SUMS>
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

# 1. The tested files, and only those: every guardiana-zero-<version>-* that SHA256SUMS names,
#    the .exe first. A file the list names but the folder lacks stops everything.
version="$(sed -n 's/^version = "\(.*\)"/\1/p' zero/navegador/Cargo.toml | head -n 1)"
nombre="guardiana-zero-$version-windows-x64.exe"
[ -f "$carpeta/$nombre" ] || { echo "release-zero.sh: $carpeta has no $nombre (zero/navegador is $version)" >&2; exit 1; }
[ -f "$carpeta/SHA256SUMS" ] || { echo "release-zero.sh: $carpeta has no SHA256SUMS" >&2; exit 1; }
# sha256sum on Windows writes «hash *name» (binary mode) and on Linux «hash  name»: both count.
nombres="$(tr -d '\r' < "$carpeta/SHA256SUMS" | grep -E "^[0-9a-f]{64} [ *]guardiana-zero-$version-" | sed -E 's/^[0-9a-f]{64} [ *]//')"
echo "$nombres" | grep -qx "$nombre" || { echo "release-zero.sh: SHA256SUMS does not name $nombre" >&2; exit 1; }
nombres="$(printf '%s\n' "$nombre"; echo "$nombres" | grep -vx "$nombre" | sort)"
for n in $nombres; do
    [ -f "$carpeta/$n" ] || { echo "release-zero.sh: SHA256SUMS names $n and $carpeta lacks it; nothing is published." >&2; exit 1; }
done
(cd "$carpeta" && tr -d '\r' < SHA256SUMS | grep -E "^[0-9a-f]{64} [ *]guardiana-zero-$version-" | sed 's/ \*/  /' | sha256sum -c -) || {
    echo "release-zero.sh: the files do not match the SHA256SUMS that came with them; nothing is published." >&2
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
for n in $nombres; do
    cp "$carpeta/$n" "$dist/$n"
done

# 2. The signatures, one per file (one password each), and the line's «files».
archivos="[]"
for n in $nombres; do
    firmar "$dist/$n" "guardiana $etiqueta $n"
    sha="$(sha256sum "$dist/$n" | cut -d' ' -f1)"
    sig="$(sed -n '2p' "$dist/$n.minisig")"
    archivos="$("$PYTHON" -c '
import json, sys
l, n, sha, sig = sys.argv[1:5]
l = json.loads(l)
l.append({"name": n, "sha256_unsigned": sha, "sha256_signed": sha, "minisign": sig, "reproducible": False})
print(json.dumps(l, separators=(",", ":")))
' "$archivos" "$n" "$sha" "$sig")"
done

# 3. The line.
line="$("$PYTHON" -c '
import json, sys
v, c, d, archivos = sys.argv[1:5]
print(json.dumps({"version": v, "commit": c, "date": d, "files": json.loads(archivos), "rekor_uuid": ""},
      separators=(",", ":")))
' "$etiqueta" "$commit" "$date" "$archivos")"

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
