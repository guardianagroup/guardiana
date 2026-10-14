#!/usr/bin/env bash
# Prepara los tres zip de la CI en Git Bash (Windows).
# Extrae con PowerShell. Convierte /c/Users/... a C:\Users\... porque PowerShell no entiende rutas MSYS.
echo "1. Empiezo (version PowerShell + cygpath)"
DL=""
for d in "$HOME/Downloads" "$HOME/Descargas" "/c/Users/franc/Downloads" "/c/Users/franc/Descargas"; do
  if [ -d "$d" ]; then DL="$d"; break; fi
done
echo "2. Carpeta: ${DL:-NO}"
if [ -z "$DL" ]; then echo "PARA: no hay carpeta de descargas"; exit 1; fi

uno() {
  local f
  for f in "$DL"/$1.zip "$DL"/$1.ZIP; do
    if [ -f "$f" ]; then echo "$f"; return 0; fi
  done
  for f in "$DL"/$1*.zip "$DL"/$1*.ZIP; do
    if [ -f "$f" ]; then echo "$f"; return 0; fi
  done
  return 1
}
L=$(uno candidato-linux || true)
M=$(uno candidato-mac || true)
W=$(uno candidato-msi || true)
echo "3. linux: ${L:-FALTA}"
echo "   mac:   ${M:-FALTA}"
echo "   msi:   ${W:-FALTA}"
if [ -z "$L" ] || [ -z "$M" ] || [ -z "$W" ]; then
  echo "PARA: falta un zip"
  ls -1 "$DL" | head -40
  exit 1
fi

# Git Bash -> Windows
win() {
  if command -v cygpath >/dev/null 2>&1; then
    cygpath -w "$1"
  else
    echo "$1" | sed 's|^/c/|C:\\|; s|/|\\|g'
  fi
}

ROOT="$DL/guardiana-1.0.27"
rm -rf "$ROOT"
mkdir -p "$ROOT/linux" "$ROOT/mac" "$ROOT/msi" "$ROOT/firmar"
echo "4. Descomprimiendo con PowerShell"
ps() {
  local src dst
  src=$(win "$1")
  dst=$(win "$2")
  powershell.exe -NoProfile -Command "Expand-Archive -LiteralPath '$src' -DestinationPath '$dst' -Force"
}
ps "$L" "$ROOT/linux" || { echo "PARA: no pude abrir el zip de linux"; exit 1; }
ps "$M" "$ROOT/mac"   || { echo "PARA: no pude abrir el zip de mac"; exit 1; }
ps "$W" "$ROOT/msi"   || { echo "PARA: no pude abrir el zip de msi"; exit 1; }
find "$ROOT/linux" "$ROOT/mac" "$ROOT/msi" -type f -name 'guardiana*' ! -name '*.minisig' -exec cp {} "$ROOT/firmar/" \;
echo "5. Archivos:"
ls -1 "$ROOT/firmar" || true
if [ -z "$(ls -A "$ROOT/firmar" 2>/dev/null)" ]; then
  echo "PARA: la carpeta quedó vacía. Los zip no se abrieron."
  exit 1
fi

huella() {
  if command -v openssl >/dev/null 2>&1; then
    openssl dgst -sha256 "$1" | awk '{print tolower($NF)}'
  else
    sha256sum -b "$1" | awk '{print tolower($1)}'
  fi
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
          line=${line%$''}
          hash=${line%% *}
          name=${line##* }
          name=${name#\*}
          if [ "$name" = "$1" ]; then echo "$hash"; return 0; fi
        done < "$sums"
      done
      ;;
  esac
}

echo "6. Comprobando uno por uno"
mal=0
lista=""
n=0
for f in "$ROOT/firmar"/guardiana*; do
  [ -f "$f" ] || continue
  n=$((n+1))
  base=$(basename "$f")
  tengo=$(huella "$f")
  quiero=$(esperada "$base")
  if [ -z "$quiero" ]; then
    echo "   SIN DATO  $base"
    mal=1
  elif [ "$tengo" = "$quiero" ]; then
    echo "   OK        $base"
    lista="$lista$quiero *$base
"
  else
    echo "   MAL       $base"
    echo "             esperado $quiero"
    echo "             obtenido $tengo"
    mal=1
  fi
done
if [ "$n" -eq 0 ] || [ "$mal" != 0 ]; then
  echo "7. HUELLAS MAL o carpeta vacía. No firmo."
  exit 1
fi

SUMS="$ROOT/firmar/SHA256SUMS"
printf '%s' "$lista" | tr -d '\r' | python -c "import sys; open(sys.argv[1],'wb').write(sys.stdin.buffer.read().replace(b'\r\n',b'\n').replace(b'\r',b'\n'))" "$SUMS"
echo "7. HUELLAS OK"
echo "   Paquetes en: $ROOT/firmar"

REPO=""
for p in \
  "$HOME/guardiana" \
  "$HOME/src/guardiana" \
  "$HOME/source/repos/guardiana" \
  "$HOME/Documents/guardiana" \
  "$HOME/Desktop/guardiana" \
  "$HOME/code/guardiana" \
  /c/Users/franc/guardiana \
  /c/Users/franc/src/guardiana \
  /c/Users/franc/source/repos/guardiana \
  /c/Users/franc/Documents/guardiana \
  /c/Users/franc/Desktop/guardiana
do
  if [ -f "$p/build/release.sh" ]; then REPO="$p"; break; fi
done
if [ -z "$REPO" ]; then
  echo "8. No encuentro la carpeta del proyecto."
  echo "Cuando la tengas, en ella pega:"
  echo "build/release.sh --paquetes \"$ROOT/firmar\""
  exit 0
fi
echo "8. Repo: $REPO"
cd "$REPO"
if ! git cat-file -e 7e1c56542be529a130dbc113a8a957d571e80edb^{commit} 2>/dev/null; then
  echo "El repo local no tiene el commit de la 1.0.27."
  echo "Los archivos ya están listos en:"
  echo "  $ROOT/firmar"
  echo "No firmo desde aquí. Cuando tengas un clon con ese commit, pega:"
  echo "build/release.sh --paquetes \"$ROOT/firmar\""
  exit 0
fi
if [ -n "$(git status --porcelain)" ]; then
  echo "Hay cambios sin guardar. No firmo."
  git status --short
  exit 0
fi
git checkout 7e1c56542be529a130dbc113a8a957d571e80edb
echo "9. Pide la contrasena de minisign.key"
bash build/release.sh --paquetes "$ROOT/firmar"
echo "10. Hecho. Si salió bien, escribe listo."
