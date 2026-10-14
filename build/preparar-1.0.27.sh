#!/usr/bin/env bash
# Prepara los tres zip de la CI (candidato-*) y, si el repo esta limpio, firma.
# No busca en todo el disco: solo en Descargas y en unas pocas carpetas.
echo "1. Empiezo"
DL=""
for d in "$HOME/Downloads" "$HOME/Descargas" "/c/Users/franc/Downloads" "/c/Users/franc/Descargas"; do
  if [ -d "$d" ]; then DL="$d"; break; fi
done
echo "2. Carpeta: ${DL:-NO}"
if [ -z "$DL" ]; then echo "PARA: no hay carpeta de descargas"; exit 1; fi

uno() {
  local f
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
  echo "PARA: falta un zip. Contenido de la carpeta:"
  ls -1 "$DL" | head -40
  exit 1
fi

ROOT="$DL/guardiana-1.0.27"
rm -rf "$ROOT"
mkdir -p "$ROOT/linux" "$ROOT/mac" "$ROOT/msi" "$ROOT/firmar"
echo "4. Descomprimiendo"
tar -xf "$L" -C "$ROOT/linux"
tar -xf "$M" -C "$ROOT/mac"
tar -xf "$W" -C "$ROOT/msi"
find "$ROOT/linux" "$ROOT/mac" "$ROOT/msi" -type f -name 'guardiana*' ! -name '*.minisig' -exec cp {} "$ROOT/firmar/" \;
echo "5. Archivos:"
ls -1 "$ROOT/firmar"

: > "$ROOT/firmar/SHA256SUMS"
while IFS= read -r sums; do
  [ -n "$sums" ] || continue
  while IFS= read -r line || [ -n "$line" ]; do
    [ -z "$line" ] && continue
    name=${line##* }
    name=${name#\*}
    if [ -f "$ROOT/firmar/$name" ]; then printf '%s\n' "$line" >> "$ROOT/firmar/SHA256SUMS"; fi
  done < "$sums"
done < <(find "$ROOT/linux" "$ROOT/mac" -name SHA256SUMS)
{
  cd "$ROOT/firmar"
  sha256sum \
    guardiana-1.0.27-windows-x64.msi \
    guardiana-1.0.27-windows-x64-en.msi \
    guardiana-1.0.27-windows-x64-pt.msi
} >> "$ROOT/firmar/SHA256SUMS"
echo "6. Comprobando"
if ! (cd "$ROOT/firmar" && sha256sum -c SHA256SUMS); then
  echo "7. HUELLAS MAL"
  exit 1
fi
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
  echo "Abre Git Bash dentro de esa carpeta y pega esta unica linea:"
  echo "build/release.sh --paquetes \"$ROOT/firmar\""
  exit 0
fi
echo "8. Repo: $REPO"
cd "$REPO"
if [ -n "$(git status --porcelain)" ]; then
  echo "Hay cambios sin guardar. No firmo."
  git status --short
  echo "Cuando este limpio, pega:"
  echo "build/release.sh --paquetes \"$ROOT/firmar\""
  exit 0
fi
git cat-file -e 7e1c56542be529a130dbc113a8a957d571e80edb^{commit} 2>/dev/null || git fetch origin
git checkout 7e1c56542be529a130dbc113a8a957d571e80edb
echo "9. Pide la contrasena de minisign.key"
bash build/release.sh --paquetes "$ROOT/firmar"
git push origin HEAD:main
echo "10. FIRMADO"
