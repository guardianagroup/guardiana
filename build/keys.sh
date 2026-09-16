#!/usr/bin/env bash
# Generate the two release keys of brief §10, OUTSIDE the repository, on the
# release machine, offline:
#   1. the minisign key pair that signs every Guardiana binary;
#   2. the Android keystore that will sign the future app.
#
# Losing either key means losing the identity of the program or of the app.
# Read docs/CLAVES.md before running this. Never run it in CI.
#
# Usage:  build/keys.sh            (keys go to $HOME/guardiana-claves)
#         GUARDIANA_KEYS=/ruta build/keys.sh
set -euo pipefail

if [ -n "${CI:-}" ] || [ -n "${GITHUB_ACTIONS:-}" ]; then
    echo "keys.sh: never run this in CI (brief §10)." >&2
    exit 1
fi

repo="$(cd "$(dirname "$0")/.." && pwd)"
out="${GUARDIANA_KEYS:-$HOME/guardiana-claves}"

case "$out" in
    "$repo"*) echo "keys.sh: the key directory must be outside the repository ($repo)." >&2; exit 1 ;;
esac

mkdir -p "$out"
chmod 700 "$out"

# ---- 1. minisign -------------------------------------------------------------
if ! command -v minisign >/dev/null 2>&1; then
    cat >&2 <<'MSG'
keys.sh: minisign is not installed.
  Debian/Ubuntu:  sudo apt install minisign
  Fedora:         sudo dnf install minisign
  macOS:          download from https://jedisct1.github.io/minisign/ (or brew install minisign)
  Windows:        download from https://jedisct1.github.io/minisign/
MSG
    exit 1
fi
if [ -e "$out/minisign.key" ]; then
    echo "keys.sh: $out/minisign.key already exists; refusing to overwrite an identity." >&2
    exit 1
fi
echo "Generating the minisign key pair. You will be asked for a password: choose a long one and keep it with the backup."
minisign -G -p "$out/minisign.pub" -s "$out/minisign.key" -c "Guardiana release key, generated $(date -u +%Y-%m-%d)"
chmod 600 "$out/minisign.key"

# Only the PUBLIC key enters the repository: it is embedded in every binary
# and its hash is the genesis of every ledger (brief §3).
cp "$out/minisign.pub" "$repo/build/pubkey/minisign.pub"
echo "Public key written to build/pubkey/minisign.pub (commit it). Private key stays in $out."

# ---- 2. Android keystore ---------------------------------------------------------
if ! command -v keytool >/dev/null 2>&1; then
    cat >&2 <<'MSG'
keys.sh: keytool (part of any Java JDK) is not installed; the Android keystore was NOT generated.
  Debian/Ubuntu:  sudo apt install default-jdk-headless
  Fedora:         sudo dnf install java-latest-openjdk-headless
  macOS/Windows:  https://adoptium.net/
Run this script again after installing it; the minisign key will not be regenerated.
MSG
    exit 2
fi
if [ -e "$out/guardiana-android.jks" ]; then
    echo "keys.sh: $out/guardiana-android.jks already exists; refusing to overwrite." >&2
    exit 1
fi
echo "Generating the Android keystore (valid 30 years). You will be asked for a password: use a different long one."
keytool -genkeypair -v \
    -keystore "$out/guardiana-android.jks" \
    -alias guardiana \
    -keyalg RSA -keysize 4096 \
    -validity 10950 \
    -dname "CN=Guardiana, O=Guardiana"
chmod 600 "$out/guardiana-android.jks"

cat <<MSG

Done. Two files now hold the identity of Guardiana:
  $out/minisign.key            (signs every binary)
  $out/guardiana-android.jks   (signs the Android app)
Back them up now, in two places, as described in docs/CLAVES.md.
MSG
