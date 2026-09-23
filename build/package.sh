#!/usr/bin/env bash
# build/package.sh: produce the installers (brief §11) from the cross-compiled
# release binaries: Linux tarball, Linux .deb (cargo-deb) and Windows MSI (WiX
# v5 as a dotnet tool, which runs on Linux/macOS too, so the MSI can be built
# inside the reproducible container; DECISIONES #34). Output in dist/<version>/
# with SHA256SUMS. Signing and the ledger line are build/release.sh's job.
#
# Needs: cargo-zigbuild + zig, cargo-deb, dotnet + `dotnet tool install -g wix`.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"
VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
OUT="dist/$VERSION"
LINUX_BIN="target/x86_64-unknown-linux-gnu/release/guardiana"
WIN_BIN="target/x86_64-pc-windows-gnu/release/guardiana.exe"
export DOTNET_ROOT="${DOTNET_ROOT:-$HOME/.dotnet}"
export DOTNET_CLI_TELEMETRY_OPTOUT=1 DOTNET_NOLOGO=1
export PATH="$HOME/.cargo/bin:$HOME/.local/zig:$DOTNET_ROOT:$DOTNET_ROOT/tools:$PATH"

if [ "${1:-}" != "--no-build" ]; then
    cargo zigbuild --release --locked -p guardiana-cli --target x86_64-unknown-linux-gnu
    cargo zigbuild --release --locked -p guardiana-cli --target x86_64-pc-windows-gnu
fi
[ -f "$LINUX_BIN" ] && [ -f "$WIN_BIN" ] || { echo "release binaries missing; run without --no-build"; exit 1; }

# Mismo sello de tiempo que build/repro.sh: sin esto, el .exe de aquí y el del contenedor no
# pueden coincidir nunca, por mucho que el código sea el mismo.
python3 build/sello-pe.py "$WIN_BIN" "$(git log -1 --format=%ct 2>/dev/null || date +%s)"

# Keep an MSI built on Windows (build/msi.ps1) that may already be in $OUT.
mkdir -p "$OUT"
find "$OUT" -type f ! -name '*.msi' -delete

# 1. Linux tarball with a readable install script.
TAR_DIR="guardiana-$VERSION-linux-x86_64"
STAGE=$(mktemp -d)
mkdir -p "$STAGE/$TAR_DIR"
cp "$LINUX_BIN" build/tarball/instalar.sh build/tarball/desinstalar.sh build/tarball/LEEME.txt "$STAGE/$TAR_DIR/"
cp docs/WHAT_IT_DOES_NOT_DO.md docs/VERIFY.md docs/HOGAR.md docs/LISTS.md LICENSE "$STAGE/$TAR_DIR/"
# Fixed mtime, owner and file order so the tarball is reproducible from the
# same inputs (GNU tar in the container, bsdtar on a Mac).
EPOCH=$(git log -1 --format=%ct 2>/dev/null || date +%s)
find "$STAGE/$TAR_DIR" -exec touch -h -d "@$EPOCH" {} + 2>/dev/null || \
    find "$STAGE/$TAR_DIR" -exec touch -h -t "$(date -r "$EPOCH" +%Y%m%d%H%M.%S)" {} +
FILES=$(cd "$STAGE" && find "$TAR_DIR" | LC_ALL=C sort)
if tar --version 2>/dev/null | grep -q GNU; then
    OWNER=(--owner=0 --group=0 --numeric-owner --mtime="@$EPOCH" --no-recursion)
else
    OWNER=(--uid 0 --gid 0 --numeric-owner -n)
fi
# shellcheck disable=SC2086
(cd "$STAGE" && tar --format=ustar "${OWNER[@]}" -cf - $FILES) | gzip -n > "$OUT/$TAR_DIR.tar.gz"
rm -rf "$STAGE"

# 2. Debian package from the already built binary.
cargo deb -p guardiana-cli --no-build --no-strip --target x86_64-unknown-linux-gnu \
    -o "$OUT/guardiana_${VERSION}_amd64.deb"

# 3. Windows MSI, one per language: whoever downloads from the English page must not be handed
# a Spanish installer (22 sep 2026). The three are the same package with the same neutral
# ProductLanguage, so one replaces another cleanly; only the words change.
# WiX 4/5 rejects Directory/@Name when it runs on Linux or macOS (it assumes a C:\ root); on
# those hosts the MSIs are built on Windows with build/msi.ps1 from the same guardiana.exe and
# added to $OUT by hand before the hashes (DECISIONES #34).
falta_msi=""
for idioma in es en pt; do
    case "$idioma" in
        es) cultura=es-ES; sufijo="" ;;
        en) cultura=en-US; sufijo="-en" ;;
        pt) cultura=pt-BR; sufijo="-pt" ;;
    esac
    salida="$OUT/guardiana-$VERSION-windows-x64$sufijo.msi"
    # Se construye a un lado y solo se mueve si sale bien. Antes se escribía directamente sobre
    # `$salida` y, al fallar (que en el Mac falla siempre, ver arriba), un `rm -f` borraba el MSI
    # que ya estaba ahí: el que se había traído de Windows. Y como release.sh llama a este guion
    # antes de firmar, el día del lanzamiento habría borrado los tres instaladores y se habría
    # parado justo después. Encontrado ensayando el 22 sep 2026.
    if wix build -arch x64 \
        -culture "$cultura" -loc "build/wix/loc-$cultura.wxl" \
        -ext WixToolset.UI.wixext -ext WixToolset.Util.wixext \
        -d "Version=$VERSION" -d "Lang=0" -d "Exe=$WIN_BIN" \
        -d "Readme=build/wix/textos/leeme-$idioma.txt" \
        -d "Welcome=build/wix/textos/bienvenida-$idioma.rtf" \
        -d "Icon=build/wix/guardiana.ico" \
        -o "$salida.nuevo" build/wix/guardiana.wxs 2>/dev/null; then
        mv "$salida.nuevo" "$salida"
    else
        rm -f "$salida.nuevo"
        falta_msi="sí"
    fi
done
if [ -n "$falta_msi" ]; then
    cp "$WIN_BIN" "$OUT/guardiana.exe"
    echo "MSI not built on this host: run build/rehacer-msi.ps1 on Windows with $OUT/guardiana.exe" >&2
    # Los que ya estén se conservan, pero hay que decir en voz alta de qué .exe tienen que venir:
    # un MSI viejo al lado del .exe de hoy es peor que no tener ninguno, porque parece que está
    # todo listo. La fecha del archivo no sirve para saberlo (se copian desde Windows y llegan con
    # la fecha de la copia), así que se dice la regla y ya.
    hay=""
    for sufijo in "" "-en" "-pt"; do
        m="$OUT/guardiana-$VERSION-windows-x64$sufijo.msi"
        [ -f "$m" ] && hay="$hay $(basename "$m")"
    done
    if [ -n "$hay" ]; then
        echo "package.sh: ya hay MSI aquí y se conservan:$hay" >&2
        echo "package.sh: tienen que estar hechos de ESTE guardiana.exe. Si no, rehazlos." >&2
    fi
fi

# 4. Hashes.
rm -f "$OUT/SHA256SUMS"
SUMS=$(mktemp)
(cd "$OUT" && if command -v sha256sum >/dev/null; then sha256sum -- *; else shasum -a 256 -- *; fi) > "$SUMS"
mv "$SUMS" "$OUT/SHA256SUMS"
echo
echo "Packages in $OUT:"
cat "$OUT/SHA256SUMS"
