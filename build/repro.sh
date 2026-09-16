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
docker run --rm \
    -v "$PWD/build/src-export:/src" \
    -v "$PWD/build/out:/out" \
    -e SOURCE_DATE_EPOCH="$epoch" \
    guardiana-repro \
    bash -euxc '
        cargo zigbuild --release --locked -p guardiana-cli --target x86_64-unknown-linux-gnu
        cargo zigbuild --release --locked -p guardiana-cli --target x86_64-pc-windows-gnu
        cp target/x86_64-unknown-linux-gnu/release/guardiana "/out/guardiana-'"$version"'-linux-x86_64"
        cp target/x86_64-pc-windows-gnu/release/guardiana.exe "/out/guardiana-'"$version"'-windows-x86_64.exe"
        chmod 644 /out/*
    '
(cd build/out && sha256sum guardiana-* > SHA256SUMS)
echo "== hashes:"
cat build/out/SHA256SUMS
