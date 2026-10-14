#!/usr/bin/env bash
# Busca en Descargas los tres zip de la 1.0.27 por tamaño exacto.
echo "1. Empiezo (por tamaño)"
DL="/c/Users/franc/Downloads"
echo "2. Zips en Descargas:"
find "$DL" -maxdepth 3 -iname '*.zip' -exec ls -l {} \;

buscar() {
  local want="$1" f sz
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    sz=$(stat -c %s "$f" 2>/dev/null || wc -c < "$f")
    if [ "$sz" = "$want" ]; then echo "$f"; return 0; fi
  done < <(find "$DL" -maxdepth 3 -iname '*.zip')
  return 1
}
L=$(buscar 34852383 || true)
M=$(buscar 11715868 || true)
W=$(buscar 60909400 || true)
echo "3. linux: ${L:-NO}"
echo "   mac:   ${M:-NO}"
echo "   msi:   ${W:-NO}"
if [ -z "$L" ] || [ -z "$M" ] || [ -z "$W" ]; then
  echo "PARA: no están los tres de la 1.0.27 (33 MB, 11 MB y 58 MB)."
  exit 1
fi

win() {
  if command -v cygpath >/dev/null 2>&1; then cygpath -w "$1"; else echo "$1" | sed 's|^/c/|C:\\|; s|/|\\|g'; fi
}
ROOT="/c/Users/franc/Downloads/guardiana-1.0.27"
rm -rf "$ROOT"
mkdir -p "$ROOT/linux" "$ROOT/mac" "$ROOT/msi" "$ROOT/firmar"
echo "4. Descomprimiendo"
ps() { powershell.exe -NoProfile -Command "Expand-Archive -LiteralPath '$(win "$1")' -DestinationPath '$(win "$2")' -Force"; }
ps "$L" "$ROOT/linux" || exit 1
ps "$M" "$ROOT/mac" || exit 1
ps "$W" "$ROOT/msi" || exit 1
find "$ROOT/linux" "$ROOT/mac" "$ROOT/msi" -type f -name 'guardiana*' ! -name '*.minisig' ! -name '*.wixpdb' -exec cp {} "$ROOT/firmar/" \;
echo "5. Archivos:"
ls -1 "$ROOT/firmar"
if ! ls "$ROOT/firmar" | grep -q '1\.0\.27'; then
  echo "PARA: esos zip no son la 1.0.27."
  exit 1
fi

huella() {
  if command -v openssl >/dev/null 2>&1; then openssl dgst -sha256 "$1" | awk '{print tolower($NF)}'; else sha256sum -b "$1" | awk '{print tolower($1)}'; fi
}
esperada() {
  case "$1" in
    guardiana-1.0.27-macos.app.zip) echo 3e961ae28601a2b680b3eb09b974cd6895ab33062e12e934bc5ba8f4a02c15e8 ;;
    guardiana_1.0.27_amd64.deb) echo 1b2fd0bf1bff9ad3ca3c04840b35510b2f7f76f6845a42535737352443e3165d ;;
    guardiana-1.0.27-windows-x86_64.exe) echo 22c901703c5c137fe115eeb956cb9d28476b515cc1e5bf8b77745dcad61fc5b8 ;;
    guardiana-1.0.27-linux-x86_64.tar.gz) echo 1360734347ad9f5de546336feac2729fac4a9483c269aee7851d36e49cebb133 ;;
    guardiana-1.0.27-windows-x64.msi) echo e1fe0960c1187adcb5ea07a90336cf0d98bcd13b509410022efecba3bba79110 ;;
    guardiana-1.0.27-windows-x64-pt.msi) echo 493c26ea8f95e1a30276d7c35f728b3c4c1928d28d031446b4456d4a6f1518f9 ;;
    guardiana-1.0.27-windows-x64-en.msi) echo bdfb22b1b3f0c131871fc0889f3fc4149317dae1113edc688b718b1cd1e907ac ;;
    *)
      local sums line name hash
      for sums in $(find "$ROOT/linux" "$ROOT/mac" -name SHA256SUMS 2>/dev/null); do
        while IFS= read -r line || [ -n "$line" ]; do
          line=${line%$''}; hash=${line%% *}; name=${line##* }; name=${name#\*}
          if [ "$name" = "$1" ]; then echo "$hash"; return 0; fi
        done < "$sums"
      done
      ;;
  esac
}
echo "6. Comprobando"
mal=0; lista=""; n=0
for f in "$ROOT/firmar"/guardiana*; do
  [ -f "$f" ] || continue
  n=$((n+1)); base=$(basename "$f"); tengo=$(huella "$f"); quiero=$(esperada "$base")
  if [ -z "$quiero" ]; then echo "   SIN DATO  $base"; mal=1
  elif [ "$tengo" = "$quiero" ]; then echo "   OK        $base"; lista="$lista$quiero *$base
"
  else echo "   MAL       $base"; echo "             esperado $quiero"; echo "             obtenido $tengo"; mal=1; fi
done
if [ "$n" -eq 0 ] || [ "$mal" != 0 ]; then echo "7. HUELLAS MAL. No firmo."; exit 1; fi
printf '%s' "$lista" | tr -d '\r' | python -c "import sys; open(sys.argv[1],'wb').write(sys.stdin.buffer.read().replace(b'\r\n',b'\n').replace(b'\r',b'\n'))" "$ROOT/firmar/SHA256SUMS"
echo "7. HUELLAS OK"

REPO="/c/Users/franc/guardiana-1027"
if [ ! -d "$REPO/.git" ]; then
  echo "8. Bajando el repo limpio (tarda)"
  git clone --depth 40 https://github.com/guardianagroup/guardiana.git "$REPO" || { echo "PARA: no pude clonar"; exit 1; }
fi
cd "$REPO"
git fetch --depth 40 origin 7e1c56542be529a130dbc113a8a957d571e80edb || true
git checkout 7e1c56542be529a130dbc113a8a957d571e80edb || { echo "PARA: no pude ir al commit de la 1.0.27"; exit 1; }
echo "9. Pide la contrasena de minisign.key"
bash build/release.sh --paquetes "$ROOT/firmar" || { echo "PARA: release.sh fallo. No esta firmado."; exit 1; }
git push origin HEAD:publicar-1.0.27 && echo "10. FIRMA SUBIDA en la rama publicar-1.0.27" || echo "10. Firmado en este PC, pero el push fallo. Copia el error."
