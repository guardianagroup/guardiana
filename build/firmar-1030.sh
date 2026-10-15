#!/usr/bin/env bash
echo "1. Empiezo a firmar la 1.0.30"
DL="/c/Users/franc/Downloads"
ROOT="/c/Users/franc/Downloads/guardiana-1.0.30"
rm -rf "$ROOT"
mkdir -p "$ROOT/linux" "$ROOT/mac" "$ROOT/msi" "$ROOT/firmar"

if command -v gh >/dev/null 2>&1; then
  echo "2. Bajo los tres paquetes"
  gh run download 38667854058 --repo guardianagroup/guardiana --dir "$ROOT/bajado" \
    && find "$ROOT/bajado" -type f -name 'guardiana*' -exec cp {} "$ROOT/firmar/" \;
fi

if [ ! -f "$ROOT/firmar/guardiana-1.0.30-linux-x86_64.tar.gz" ]; then
  buscar() {
    local want="$1" f sz
    while IFS= read -r f; do
      [ -n "$f" ] || continue
      sz=$(stat -c %s "$f" 2>/dev/null || wc -c < "$f")
      if [ "$sz" = "$want" ]; then echo "$f"; return 0; fi
    done < <(find "$DL" -maxdepth 2 -iname 'candidato-*.zip')
    return 1
  }
  L=$(buscar 53835009 || true)
  M=$(buscar 11714621 || true)
  W=$(buscar 115932801 || true)
  echo "2. linux: ${L:-NO}"
  echo "   mac:   ${M:-NO}"
  echo "   msi:   ${W:-NO}"
  if [ -z "$L" ] || [ -z "$M" ] || [ -z "$W" ]; then
    echo "PARA: descarga los tres (linux, mac, msi) desde"
    echo "https://github.com/guardianagroup/guardiana/actions/runs/38667854058"
    echo "y vuelve a pegar esta misma linea."
    exit 1
  fi
  win() { if command -v cygpath >/dev/null 2>&1; then cygpath -w "$1"; else echo "$1" | sed 's|^/c/|C:\\|; s|/|\\|g'; fi; }
  echo "3. Descomprimiendo"
  ps() { powershell.exe -NoProfile -Command "Expand-Archive -LiteralPath '$(win "$1")' -DestinationPath '$(win "$2")' -Force"; }
  ps "$L" "$ROOT/linux" || exit 1
  ps "$M" "$ROOT/mac" || exit 1
  ps "$W" "$ROOT/msi" || exit 1
  find "$ROOT/linux" "$ROOT/mac" "$ROOT/msi" -type f -name 'guardiana*' ! -name '*.wixpdb' -exec cp {} "$ROOT/firmar/" \;
fi

tar -xOf "$ROOT/firmar/guardiana-1.0.30-linux-x86_64.tar.gz" guardiana-1.0.30-linux-x86_64/guardiana > "$ROOT/firmar/guardiana-1.0.30-linux-x86_64"
chmod +x "$ROOT/firmar/guardiana-1.0.30-linux-x86_64"
echo "4. Archivos:"
ls -1 "$ROOT/firmar"

huella() {
  if command -v openssl >/dev/null 2>&1; then openssl dgst -sha256 "$1" | awk '{print tolower($NF)}'; else sha256sum -b "$1" | awk '{print tolower($1)}'; fi
}
esperada() {
  case "$1" in
    guardiana-1.0.30-macos.app.zip) echo 30c597a6eb3e1b46ae0992b5a53848b069169ccaa498e76b9a50601cef1d5bed ;;
    guardiana_1.0.30_amd64.deb) echo fb4a4ce28c88c4b51ccebb39802a513f958fb5192c5619d80719f4d16039b8c8 ;;
    guardiana-1.0.30-windows-x86_64.exe) echo 987b8944e584add9eab2ada6860ce2cab9c5265e814f09e4faf2835343c94b4b ;;
    guardiana-1.0.30-linux-x86_64.tar.gz) echo 26a715f999afe5baaae6dd97b61345b5a3677e980eb6e664cb008b22b09630ff ;;
    guardiana-1.0.30-linux-x86_64) echo abbc6c7718556417d5e8a35c717c995e8077fa4b4e20622e068dd0acdac8dd8c ;;
    guardiana-1.0.30-windows-x64.msi) echo 11d19206a6b459f6c19a9a0be6ad01865e793816a7d1015c4a9db549c45294e1 ;;
    guardiana-1.0.30-windows-x64-pt.msi) echo 7842ca0995b662b5f28ef226b8bdb68a386a5c76af00571fe48de442764ae21e ;;
    guardiana-1.0.30-windows-x64-en.msi) echo eaddb93e329a5eb6be89ad38ae9a01a13593c5feec4ba99eaa54f596e66dfc52 ;;
  esac
}
echo "5. Comprobando"
mal=0; n=0
: > "$ROOT/firmar/SHA256SUMS"
for f in "$ROOT/firmar"/guardiana*; do
  [ -f "$f" ] || continue
  n=$((n+1)); base=$(basename "$f"); tengo=$(huella "$f")
  quiero=$(esperada "$base")
  if [ -z "$quiero" ]; then echo "   SIN DATO  $base"; mal=1
  elif [ "$tengo" = "$quiero" ]; then
    echo "   OK        $base"
    printf '%s *%s\n' "$quiero" "$base" >> "$ROOT/firmar/SHA256SUMS"
  else
    echo "   MAL       $base"
    echo "             esperado $quiero"
    echo "             obtenido $tengo"
    mal=1
  fi
done
if [ "$n" -eq 0 ] || [ "$mal" != 0 ]; then echo "6. HUELLAS MAL. No firmo."; exit 1; fi
echo "6. HUELLAS OK"
REPO="/c/Users/franc/guardiana-1030"
if [ ! -d "$REPO/.git" ]; then
  git clone --branch candidato-1.0.30 https://github.com/guardianagroup/guardiana.git "$REPO"
fi
cd "$REPO"
git fetch --depth 1 origin 2777cc68ed4f84be2f058d05551aeee12e1547c8
git checkout --detach 2777cc68ed4f84be2f058d05551aeee12e1547c8
git config user.email "francistierraplana@icloud.com"
git config user.name "guardianagroup"
echo "7. Pide la contrasena. Es la misma, y puede pedirla varias veces."
bash build/release.sh --paquetes "$ROOT/firmar" || { echo "PARA: release.sh fallo. No esta firmado."; exit 1; }
git push origin HEAD:refs/heads/publicar-1.0.30 && echo "8. FIRMA SUBIDA" || echo "8. Firmado en este PC, pero el push fallo. Copia el error."
