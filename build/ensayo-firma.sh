#!/usr/bin/env bash
# Ensayo de la firma: firma los paquetes de una versión y comprueba cada firma, sin tocar el
# registro público ni subir nada. Es el paso que el 5 de octubre hará `build/release.sh`, hecho
# antes y por separado para que el día del lanzamiento no haya ninguna sorpresa.
#
#   bash build/ensayo-firma.sh [carpeta]     (por defecto dist/<versión de Cargo.toml>)
#
# Pide la contraseña de la clave una vez por archivo: la escribe la persona, aquí y en su terminal.
# Lo que NO hace: no escribe en ledger.jsonl, no sube nada a Rekor, no publica, no borra.
set -euo pipefail
cd "$(dirname "$0")/.."

version="$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')"
carpeta="${1:-dist/$version}"
claves="${GUARDIANA_KEYS:-$HOME/guardiana-claves}"
publica="build/pubkey/minisign.pub"

[ -d "$carpeta" ] || { echo "ensayo-firma: no existe la carpeta $carpeta" >&2; exit 1; }
[ -f "$claves/minisign.key" ] || { echo "ensayo-firma: no está $claves/minisign.key" >&2; exit 1; }
if command -v minisign >/dev/null 2>&1; then
    firmar() { minisign -Sm "$1" -s "$claves/minisign.key" -t "$2"; }
    comprobar() { minisign -Vm "$1" -p "$publica"; }
elif command -v rsign >/dev/null 2>&1 || [ -x "$HOME/.cargo/bin/rsign" ]; then
    rs="$(command -v rsign || echo "$HOME/.cargo/bin/rsign")"
    firmar() { "$rs" sign -s "$claves/minisign.key" -x "$1.minisig" -t "$2" "$1"; }
    comprobar() { "$rs" verify -p "$publica" -x "$1.minisig" "$1"; }
else
    echo "ensayo-firma: hace falta minisign o rsign (cargo install rsign2)" >&2
    exit 1
fi

archivos=()
while IFS= read -r f; do archivos+=("$f"); done < <(find "$carpeta" -maxdepth 1 -type f \
    ! -name '*.minisig' ! -name 'SHA256SUMS' | sort)
[ "${#archivos[@]}" -gt 0 ] || { echo "ensayo-firma: no hay nada que firmar en $carpeta" >&2; exit 1; }

echo "== Voy a firmar ${#archivos[@]} archivos de la versión $version:"
for f in "${archivos[@]}"; do echo "   $(basename "$f")"; done
echo "== La contraseña se pide una vez por archivo. No se escribe en ningún registro ni se sube nada."
echo

for f in "${archivos[@]}"; do
    echo "-- firmando $(basename "$f")"
    firmar "$f" "guardiana $version $(basename "$f")"
done

echo
echo "== Comprobando cada firma con la clave pública ($publica):"
fallos=0
for f in "${archivos[@]}"; do
    if comprobar "$f" >/dev/null 2>&1; then
        echo "   OK   $(basename "$f")"
    else
        echo "   MAL  $(basename "$f")"; fallos=$((fallos + 1))
    fi
done

echo
if command -v sha256sum >/dev/null 2>&1; then suma="sha256sum"; else suma="shasum -a 256"; fi
# Solo los paquetes: las firmas no se firman a sí mismas, y meterlas aquí era ruido.
(cd "$carpeta" && rm -f SHA256SUMS && $suma $(ls guardiana* | grep -v '\.minisig$') > SHA256SUMS)
echo "== Huellas en $carpeta/SHA256SUMS:"
cat "$carpeta/SHA256SUMS"
echo
if [ "$fallos" -eq 0 ]; then
    echo "== Ensayo correcto: ${#archivos[@]} archivos firmados y las ${#archivos[@]} firmas comprueban."
    echo "== No se ha publicado nada. El registro público se escribe el día del lanzamiento, con build/release.sh."
else
    echo "== $fallos firmas NO comprueban. No se publica nada hasta entender por qué." >&2
    exit 1
fi
