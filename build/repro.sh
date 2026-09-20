#!/usr/bin/env bash
# Reproducible build (brief §10): anyone with Docker runs this on a commit and
# must obtain the same SHA-256 hashes that ledger.jsonl publishes as
# sha256_unsigned. Output: build/out/<files> and build/out/SHA256SUMS.
#
# Usage: build/repro.sh [git-ref]      (default: HEAD)
set -euo pipefail

cd "$(dirname "$0")/.."
ref="${1:-HEAD}"
commit="$(git rev-parse "$ref")"
version="$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')"
epoch="$(git show -s --format=%ct "$commit")"

command -v docker >/dev/null 2>&1 || { echo "repro.sh: docker is required" >&2; exit 1; }

echo "== Guardiana $version, commit $commit, SOURCE_DATE_EPOCH=$epoch"
rm -rf build/out build/src-export
mkdir -p build/out build/src-export
# Export the exact commit, not the working tree, so local changes never leak in.
git archive --format=tar "$commit" | tar -x -C build/src-export

docker build -t guardiana-repro -f build/Dockerfile build
# El código entra de solo lectura y lo que compila vive dentro del contenedor: así la compilación
# no puede tocar el árbol exportado, y sobre todo no lo deja lleno de archivos de root que la
# segunda pasada no puede borrar. Eso es lo que hacía fallar la comprobación de «dos veces da lo
# mismo» en el flujo de GitHub el 20 sep 2026, antes de haber comparado ninguna huella.
docker run --rm \
    -v "$PWD/build/src-export:/src:ro" \
    -v "$PWD/build/out:/out" \
    -e SOURCE_DATE_EPOCH="$epoch" \
    -e CARGO_TARGET_DIR=/tmp/target \
    guardiana-repro \
    bash -euxc '
        cargo zigbuild --release --locked -p guardiana-cli --target x86_64-unknown-linux-gnu
        cargo zigbuild --release --locked -p guardiana-cli --target x86_64-pc-windows-gnu
        cp /tmp/target/x86_64-unknown-linux-gnu/release/guardiana "/out/guardiana-'"$version"'-linux-x86_64"
        cp /tmp/target/x86_64-pc-windows-gnu/release/guardiana.exe "/out/guardiana-'"$version"'-windows-x86_64.exe"
        chmod 644 /out/*
    '
# El sello de tiempo que el enlazador mete en el .exe es el reloj de la máquina, así que dos
# compilaciones del mismo código daban dos huellas distintas (visto el 20 sep 2026, la primera vez
# que esto se pudo correr dos veces seguidas). Se reescribe con la fecha del commit.
python3 build/sello-pe.py "build/out/guardiana-$version-windows-x86_64.exe" "$epoch"

(cd build/out && sha256sum guardiana-* > SHA256SUMS)
echo "== hashes:"
cat build/out/SHA256SUMS
