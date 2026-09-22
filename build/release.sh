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

# 1 bis. Los paquetes que la gente descarga de verdad. La página de instalación dice, con estas
# palabras, «compara la huella del .msi con la del registro público», y hasta el 20 de septiembre
# de 2026 el registro solo llevaba los dos binarios sueltos: quien siguiera la instrucción el día
# del lanzamiento no habría encontrado su archivo. Los envoltorios se construyen desde los mismos
# binarios del contenedor y entran en la misma línea, cada uno diciendo si es reproducible.
cp "$linux" target/x86_64-unknown-linux-gnu/release/guardiana
cp "$win" target/x86_64-pc-windows-gnu/release/guardiana.exe
build/package.sh --no-build
paquetes="dist/$version"
# Un instalador por idioma (22 sep 2026): el mismo paquete con las mismas palabras que la web
# de ese idioma. Los tres van al registro público, porque los tres se descargan.
msi="$paquetes/guardiana-$version-windows-x64.msi"
msi_en="$paquetes/guardiana-$version-windows-x64-en.msi"
msi_pt="$paquetes/guardiana-$version-windows-x64-pt.msi"
deb="$paquetes/guardiana_${version}_amd64.deb"
tarball="$paquetes/guardiana-$version-linux-x86_64.tar.gz"
for m in "$msi" "$msi_en" "$msi_pt"; do
    if [ ! -f "$m" ]; then
        echo "release.sh: falta $m." >&2
        echo "release.sh: los MSI se construyen en Windows (build/msi.ps1 -Idioma es|en|pt) desde este" >&2
        echo "release.sh: mismo .exe y se dejan ahí antes de publicar, porque la web dice que su huella" >&2
        echo "release.sh: está en el registro." >&2
        exit 1
    fi
done

# 2. minisign signatures over everything that gets published.
for f in "$linux" "$win" "$msi" "$msi_en" "$msi_pt" "$deb" "$tarball"; do
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

# 5. The ledger line: one entry per file that anyone can download, not only the two raw binaries.
# `reproducible` says the truth about each one: the two binaries come out of the pinned container
# and two builds of the same commit give the same hash (comprobado el 20 sep 2026, después de fijar
# el sello de tiempo del .exe); los envoltorios —MSI, .deb, tarball, .app.zip— se construyen una
# vez y lo que se publica de ellos es su huella y su firma, que es lo que la persona compara.
entradas=""
anotar() {  # archivo  reproducible(true|false)  [huella ya firmada]
    local f="$1" repro="$2" firmado="${3:-}" sha sig fila
    sha="$(sha256sum "$f" | cut -d' ' -f1)"
    sig="$(sed -n '2p' "$f.minisig")"
    # printf -v y no $( ), que se come el salto de línea final y dejaría todo en una sola fila.
    printf -v fila '%s\t%s\t%s\t%s\t%s\n' "$(basename "$f")" "$sha" "${firmado:-$sha}" "$sig" "$repro"
    entradas="$entradas$fila"
}
anotar "$linux" true
anotar "$win" true "$sha_win_signed"
anotar "$msi" false
anotar "$msi_en" false
anotar "$msi_pt" false
anotar "$deb" false
anotar "$tarball" false
[ -n "$mac" ] && anotar "$mac" false

line=$(printf '%s' "$entradas" | python3 -c '
import json, sys
v, c, d = sys.argv[1:4]
files = []
for linea in sys.stdin.read().splitlines():
    if not linea.strip():
        continue
    nombre, sha, firmado, sig, repro = linea.split("\t")
    files.append({"name": nombre, "sha256_unsigned": sha, "sha256_signed": firmado,
                  "minisign": sig, "reproducible": repro == "true"})
print(json.dumps({"version": v, "commit": c, "date": d, "files": files, "rekor_uuid": ""},
                 separators=(",", ":")))
' "$version" "$commit" "$date")

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
