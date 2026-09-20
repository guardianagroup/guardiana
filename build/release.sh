#!/usr/bin/env bash
# Publish a version (brief §10), in this exact order:
#   1. reproducible build (build/repro.sh) → SHA-256 of each binary (unsigned)
#      plus, on a Mac, GUARDIANA.app zipped (it cannot be cross-built)
#   2. minisign signature of each binary with the release key (never in CI)
#   3. Windows code signing (optional; fails clearly if not configured; timestamped)
#   4. SHA-256 of the signed Windows binary
#   5. append the line to ledger.jsonl
#   6. upload that line to Rekor
#   7. commit ledger.jsonl — only then may the download be published
#
# Environment:
#   GUARDIANA_KEYS      directory with minisign.key (default $HOME/guardiana-claves)
#   CERTUM_CERT_SHA1    thumbprint of the GUARDIANA GROUP certificate in the Windows store
#                       (Certum SimplySign, cloud); CERTUM_SIGNTOOL points at signtool.exe
#   ESIGNER_USERNAME, ESIGNER_PASSWORD, ESIGNER_TOTP, ESIGNER_CREDENTIAL_ID
#                       SSL.com eSigner (CodeSignTool) for the Windows signature; optional
#   CODESIGNTOOL        path to CodeSignTool.sh (optional)
set -euo pipefail

if [ -n "${CI:-}" ] || [ -n "${GITHUB_ACTIONS:-}" ]; then
    echo "release.sh: never run in CI: the private key must not be there (brief §10)." >&2
    exit 1
fi

cd "$(dirname "$0")/.."
keys="${GUARDIANA_KEYS:-$HOME/guardiana-claves}"
version="$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')"
commit="$(git rev-parse HEAD)"
date="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

[ -f "$keys/minisign.key" ] || { echo "release.sh: no $keys/minisign.key (run build/keys.sh on the release machine)" >&2; exit 1; }
# The signer: minisign, or rsign (cargo install rsign2), which writes the same format.
# The release Mac has no Homebrew, so minisign is not installable there; rsign is, and
# it was the one used for the 16 sep 2026 rehearsal (decision 120).
if command -v minisign >/dev/null 2>&1; then
    firmar() { minisign -Sm "$1" -s "$keys/minisign.key" -t "$2"; }
elif command -v rsign >/dev/null 2>&1; then
    firmar() { rsign sign -s "$keys/minisign.key" -x "$1.minisig" -t "$2" "$1"; }
else
    echo "release.sh: minisign or rsign is required (cargo install rsign2)" >&2
    exit 1
fi
if grep -q DEV-NOT-A-KEY build/pubkey/minisign.pub; then
    echo "release.sh: build/pubkey/minisign.pub is the development placeholder; run build/keys.sh first" >&2
    exit 1
fi
if [ -n "$(git status --porcelain)" ]; then
    echo "release.sh: the working tree must be clean" >&2
    exit 1
fi

# 1. Reproducible build.
build/repro.sh HEAD
out="build/out"
linux="$out/guardiana-$version-linux-x86_64"
win="$out/guardiana-$version-windows-x86_64.exe"

# 2. minisign signatures over the unsigned binaries.
for f in "$linux" "$win"; do
    firmar "$f" "guardiana $version $(basename "$f")"
done
sha_linux="$(sha256sum "$linux" | cut -d' ' -f1)"

# 2 bis. macOS: the .app can only be built on a Mac, so this step runs when the release machine
# is one and is skipped, loudly, when it is not. The app is not notarised (Apple asks for a paid
# developer account): the person sees Gatekeeper's warning and the install page explains it, the
# same way the Windows one is explained. What replaces notarisation here is the same thing as on
# Linux: the hash and the minisign signature, published before the download exists.
mac=""; sha_mac=""; sig_mac=""
if [ "$(uname)" = "Darwin" ]; then
    cargo build --release --locked -p guardiana-cli
    build/mac/crear-app.sh "$out/app" >/dev/null
    mac="$out/guardiana-$version-macos.app.zip"
    rm -f "$mac"
    # -y stores the symlinks inside the bundle instead of following them.
    (cd "$out/app" && zip -qry "../$(basename "$mac")" GUARDIANA.app)
    firmar "$mac" "guardiana $version $(basename "$mac")"
    sha_mac="$(sha256sum "$mac" | cut -d' ' -f1)"
    sig_mac="$(sed -n '2p' "$mac.minisig")"
else
    echo "release.sh: not on a Mac, so no macOS app is built. Run this step on the Mac before publishing." >&2
fi
sha_win_unsigned="$(sha256sum "$win" | cut -d' ' -f1)"

# 3. Windows code signing, optional, always timestamped.
# Two providers, both in the cloud (no USB token, so the release machine can be any of ours):
#   CERTUM_*    Certum Cloud Code Signing with SimplySign, the certificate of GUARDIANA GROUP
#               (decision 66). Signs with signtool/osslsigncode against the cloud key.
#   CODESIGNTOOL SSL.com eSigner, kept as the earlier alternative.
# Whichever is used, the publisher shown to the user is the organisation on the certificate, and
# the timestamp is always applied so signatures outlive the certificate.
sha_win_signed="$sha_win_unsigned"
if [ -n "${CERTUM_CERT_SHA1:-}" ]; then
    : "${CERTUM_SIGNTOOL:?CERTUM_SIGNTOOL required: path to signtool.exe (Windows SDK) on the signing machine}"
    mkdir -p "$out/signed"
    cp "$win" "$out/signed/"
    # SimplySign publishes the cloud key as a Windows certificate store entry; /sha1 picks it.
    "$CERTUM_SIGNTOOL" sign /sha1 "$CERTUM_CERT_SHA1" /fd sha256 /td sha256 \
        /tr "${CERTUM_TIMESTAMP:-http://time.certum.pl}" /d "GUARDIANA" /du "https://guardianagroup.com" \
        "$out/signed/$(basename "$win")"
    "$CERTUM_SIGNTOOL" verify /pa /v "$out/signed/$(basename "$win")"
    sha_win_signed="$(sha256sum "$out/signed/$(basename "$win")" | cut -d' ' -f1)"
elif [ -n "${CODESIGNTOOL:-}" ]; then
    : "${ESIGNER_USERNAME:?ESIGNER_USERNAME required}"
    : "${ESIGNER_PASSWORD:?ESIGNER_PASSWORD required}"
    : "${ESIGNER_TOTP:?ESIGNER_TOTP required}"
    : "${ESIGNER_CREDENTIAL_ID:?ESIGNER_CREDENTIAL_ID required}"
    mkdir -p "$out/signed"
    "$CODESIGNTOOL" sign -username="$ESIGNER_USERNAME" -password="$ESIGNER_PASSWORD" \
        -totp_secret="$ESIGNER_TOTP" -credential_id="$ESIGNER_CREDENTIAL_ID" \
        -input_file_path="$win" -output_dir_path="$out/signed"
    sha_win_signed="$(sha256sum "$out/signed/$(basename "$win")" | cut -d' ' -f1)"
else
    echo "release.sh: neither CERTUM_CERT_SHA1 nor CODESIGNTOOL set: the Windows binary is NOT code-signed." >&2
    echo "release.sh: Windows will warn until the signature gathers reputation; see docs/FIRMA-CODIGO.md." >&2
fi

# 5. The ledger line.
sig_linux="$(sed -n '2p' "$linux.minisig")"
sig_win="$(sed -n '2p' "$win.minisig")"
line=$(python3 - "$version" "$commit" "$date" "$linux" "$sha_linux" "$sig_linux" "$win" "$sha_win_unsigned" "$sha_win_signed" "$sig_win" "$mac" "$sha_mac" "$sig_mac" <<'PY'
import json, sys, os
v, c, d, lx, shl, sgl, wn, shwu, shws, sgw, mc, shm, sgm = sys.argv[1:]
files = [
  {"name": os.path.basename(lx), "sha256_unsigned": shl, "sha256_signed": shl, "minisign": sgl},
  {"name": os.path.basename(wn), "sha256_unsigned": shwu, "sha256_signed": shws, "minisign": sgw},
]
if mc:
    # The Mac app is not code-signed by Apple, so signed and unsigned are the same file.
    files.append({"name": os.path.basename(mc), "sha256_unsigned": shm, "sha256_signed": shm, "minisign": sgm})
print(json.dumps({"version": v, "commit": c, "date": d, "files": files, "rekor_uuid": ""}, separators=(",", ":")))
PY
)

# 6. Rekor: sign the line itself and upload; the uuid goes into the line.
echo "$line" > "$out/ledger-line.json"
firmar "$out/ledger-line.json" "guardiana $version ledger line"
rekor_uuid=""
if command -v rekor-cli >/dev/null 2>&1; then
    rekor_uuid="$(rekor-cli upload --artifact "$out/ledger-line.json" --signature "$out/ledger-line.json.minisig" \
        --pki-format minisign --public-key build/pubkey/minisign.pub --format json | python3 -c 'import json,sys; print(json.load(sys.stdin).get("Location","").rsplit("/",1)[-1])')"
    line="$(python3 -c 'import json,sys; l=json.loads(sys.argv[1]); l["rekor_uuid"]=sys.argv[2]; print(json.dumps(l, separators=(",",":")))' "$line" "$rekor_uuid")"
else
    # No rekor-cli: the public log takes the same entry over its REST API, which is
    # all this needs and one dependency less. Rehearsed on 16 sep 2026 (decision 120);
    # note that the staging instance refuses "rekord" entries, so a rehearsal that
    # wants a real uuid has to use the production log, like the release does.
    rekor_uuid="$(python3 - "$out/ledger-line.json" "$out/ledger-line.json.minisig" build/pubkey/minisign.pub <<'PYREKOR'
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
    print("release.sh: Rekor refused the entry: %s %s" % (e.code, e.read()[:200].decode("utf-8", "replace")), file=sys.stderr)
PYREKOR
)"
    [ -n "$rekor_uuid" ] || echo "release.sh: no rekor uuid; upload $out/ledger-line.json by hand before publishing." >&2
    line="$(python3 -c 'import json,sys; l=json.loads(sys.argv[1]); l["rekor_uuid"]=sys.argv[2]; print(json.dumps(l, separators=(",",":")))' "$line" "$rekor_uuid")"
fi

# 7. Append and commit. The download is published only after this commit is pushed.
echo "$line" >> ledger.jsonl
git add ledger.jsonl
git commit -m "Publish $version to ledger.jsonl (rekor ${rekor_uuid:-pending})"
echo "== ledger line:"
echo "$line"
echo "== now push, then upload the files in $out (and $out/signed) to the web. Never before."
