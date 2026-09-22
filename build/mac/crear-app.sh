#!/bin/sh
# Builds GUARDIANA.app for the development Mac and leaves a copy on the Desktop.
# macOS ships with 1.0: this is the .app that release.sh zips, signs and publishes. The old
# note said it was an internal tool because macOS used to be out of 1.0 (decision 55); it came
# in later and the note stayed until 22 sep 2026.
#   build/mac/crear-app.sh [output folder, default dist/mac]
set -e
cd "$(dirname "$0")/../.."
OUT="${1:-dist/mac}"
APP="$OUT/GUARDIANA.app"
cargo build --release --locked -p guardiana-cli
rm -rf "$APP"; mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp build/mac/Info.plist "$APP/Contents/Info.plist"
cp build/mac/lanzador.sh "$APP/Contents/MacOS/GUARDIANA"
cp build/mac/instalar.sh build/mac/desinstalar.sh "$APP/Contents/Resources/"
cp target/release/guardiana "$APP/Contents/Resources/guardiana"
chmod 755 "$APP/Contents/MacOS/GUARDIANA" "$APP/Contents/Resources/"*.sh "$APP/Contents/Resources/guardiana"
# Icon from the site's 512 px logo, with the tools macOS ships (sips, iconutil).
SET="$(mktemp -d)/guardiana.iconset"; mkdir -p "$SET"
for s in 16 32 128 256 512; do
  sips -z "$s" "$s" site/icon-512.png --out "$SET/icon_${s}x${s}.png" >/dev/null
  d=$((s * 2)); [ "$d" -le 1024 ] && sips -z "$d" "$d" site/icon-512.png --out "$SET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$SET" -o "$APP/Contents/Resources/guardiana.icns"
rm -rf "$(dirname "$SET")"
touch "$APP"
echo "hecho: $APP"
