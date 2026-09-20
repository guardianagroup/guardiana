#!/usr/bin/env bash
# Hace una clave de firma nueva y comprueba, en el momento, que la contraseña que acabas de poner
# funciona. La anterior no se borra: se aparta con la fecha, por si aparece su contraseña.
#
#   bash build/clave-nueva.sh
#
# Lo único que tienes que hacer tú es escribir la contraseña. Tres veces seguidas: dos para crearla
# y una para probarla. Escríbela antes en tu gestor de contraseñas; si se pierde, la clave no sirve
# y hay que repetir esto.
set -euo pipefail
cd "$(dirname "$0")/.."

claves="${GUARDIANA_KEYS:-$HOME/guardiana-claves}"
rs="$(command -v rsign || echo "$HOME/.cargo/bin/rsign")"
[ -x "$rs" ] || { echo "clave-nueva: falta rsign (cargo install rsign2)" >&2; exit 1; }
mkdir -p "$claves"; chmod 700 "$claves"

hoy="$(date +%Y%m%d-%H%M)"
if [ -f "$claves/minisign.key" ]; then
    mv "$claves/minisign.key" "$claves/minisign-$hoy.key.vieja"
    [ -f "$claves/minisign.pub" ] && mv "$claves/minisign.pub" "$claves/minisign-$hoy.pub.vieja"
    echo "== La clave anterior queda guardada como minisign-$hoy.key.vieja (no se borra nada)."
fi

cat <<'AVISO'

== Se va a crear una clave de firma nueva para GUARDIANA.

   Escribe la contraseña en tu gestor de contraseñas ANTES de teclearla aquí.
   Guárdala con el nombre: GUARDIANA · clave de firma.
   Sin ella la clave no sirve y hay que repetir esto.

   Ahora la pedirá dos veces (no se ve nada al escribir: es normal).

AVISO

"$rs" generate -s "$claves/minisign.key" -p "$claves/minisign.pub" -f \
    -c "Guardiana release key, generada $(date -u +%Y-%m-%d)"
chmod 600 "$claves/minisign.key"

echo
echo "== Prueba: voy a firmar un archivo de mentira para comprobar que la contraseña funciona."
echo "   La pedirá una vez más."
echo
prueba="$(mktemp -d)/prueba.txt"
echo "archivo de prueba de GUARDIANA" > "$prueba"
if "$rs" sign -s "$claves/minisign.key" -x "$prueba.minisig" -t "prueba de la clave nueva" "$prueba"; then
    if "$rs" verify -p "$claves/minisign.pub" -x "$prueba.minisig" "$prueba" >/dev/null 2>&1; then
        cp "$claves/minisign.pub" build/pubkey/minisign.pub
        echo
        echo "== LISTO. La contraseña funciona y la firma comprueba."
        echo "== Clave pública nueva (ya copiada a build/pubkey/minisign.pub):"
        echo
        cat build/pubkey/minisign.pub
        echo
        echo "== Dile a Claude que ya está: él cambia la clave en la web, en el programa y en el"
        echo "== repositorio, y repite el ensayo de firma."
    else
        echo "== La firma no comprueba. Algo raro pasa: no sigas y dímelo." >&2
        exit 1
    fi
else
    echo "== No se pudo firmar: la contraseña no coincide con la que acabas de poner." >&2
    echo "== Vuelve a lanzar este mismo comando y ponla de nuevo, con calma." >&2
    exit 1
fi
rm -rf "$(dirname "$prueba")"
