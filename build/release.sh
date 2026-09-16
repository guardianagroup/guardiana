#!/usr/bin/env bash
# Publish a version (brief §10), in this exact order:
#   1. reproducible build (build/repro.sh) → SHA-256 of each binary (unsigned)
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
command -v minisign >/dev/null 2>&1 || { echo "release.sh: minisign is required" >&2; exit 1; }
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
    minisign -Sm "$f" -s "$keys/minisign.key" -t "guardiana $version $(basename "$f")"
done
sha_linux="$(sha256sum "$linux" | cut -d' ' -f1)"
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
line=$(python3 - "$version" "$commit" "$date" "$linux" "$sha_linux" "$sig_linux" "$win" "$sha_win_unsigned" "$sha_win_signed" "$sig_win" <<'PY'
import json, sys, os
v, c, d, lx, shl, sgl, wn, shwu, shws, sgw = sys.argv[1:]
print(json.dumps({"version": v, "commit": c, "date": d, "files": [
  {"name": os.path.basename(lx), "sha256_unsigned": shl, "sha256_signed": shl, "minisign": sgl},
  {"name": os.path.basename(wn), "sha256_unsigned": shwu, "sha256_signed": shws, "minisign": sgw},
], "rekor_uuid": ""}, separators=(",", ":")))
PY
)

# 6. Rekor: sign the line itself and upload; the uuid goes into the line.
echo "$line" > "$out/ledger-line.json"
minisign -Sm "$out/ledger-line.json" -s "$keys/minisign.key"
rekor_uuid=""
if command -v rekor-cli >/dev/null 2>&1; then
    rekor_uuid="$(rekor-cli upload --artifact "$out/ledger-line.json" --signature "$out/ledger-line.json.minisig" \
        --pki-format minisign --public-key build/pubkey/minisign.pub --format json | python3 -c 'import json,sys; print(json.load(sys.stdin).get("Location","").rsplit("/",1)[-1])')"
    line="$(python3 -c 'import json,sys; l=json.loads(sys.argv[1]); l["rekor_uuid"]=sys.argv[2]; print(json.dumps(l, separators=(",",":")))' "$line" "$rekor_uuid")"
else
    echo "release.sh: rekor-cli not found; upload $out/ledger-line.json by hand and fill rekor_uuid before publishing." >&2
fi

# 7. Append and commit. The download is published only after this commit is pushed.
echo "$line" >> ledger.jsonl
git add ledger.jsonl
git commit -m "Publish $version to ledger.jsonl (rekor ${rekor_uuid:-pending})"
echo "== ledger line:"
echo "$line"
echo "== now push, then upload the files in $out (and $out/signed) to the web. Never before."
