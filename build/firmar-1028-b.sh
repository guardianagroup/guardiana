#!/usr/bin/env bash
echo "1. Empiezo a firmar la 1.0.28"
ROOT="/c/Users/franc/Downloads/guardiana-1.0.28"
rm -rf "$ROOT"
mkdir -p "$ROOT/firmar"
BASE="https://github.com/guardianagroup/guardiana/releases/download/candidatos-1.0.28"
echo "2. Bajando los paquetes"
for f in \
  guardiana-1.0.28-linux-x86_64.tar.gz \
  guardiana-1.0.28-windows-x86_64.exe \
  guardiana_1.0.28_amd64.deb \
  guardiana-1.0.28-macos.app.zip \
  guardiana-1.0.28-windows-x64.msi \
  guardiana-1.0.28-windows-x64-en.msi \
  guardiana-1.0.28-windows-x64-pt.msi
do
  echo "   $f"
  curl -fL --retry 3 -o "$ROOT/firmar/$f" "$BASE/$f" || { echo "PARA: no pude bajar $f"; exit 1; }
done
tar -xOf "$ROOT/firmar/guardiana-1.0.28-linux-x86_64.tar.gz" guardiana-1.0.28-linux-x86_64/guardiana > "$ROOT/firmar/guardiana-1.0.28-linux-x86_64"
chmod +x "$ROOT/firmar/guardiana-1.0.28-linux-x86_64"
echo "3. Archivos:"
ls -1 "$ROOT/firmar"

huella() {
  if command -v openssl >/dev/null 2>&1; then openssl dgst -sha256 "$1" | awk '{print tolower($NF)}'; else sha256sum -b "$1" | awk '{print tolower($1)}'; fi
}
esperada() {
  case "$1" in
    guardiana-1.0.28-macos.app.zip) echo 11adb67558c953570cb576633f05b6a06db68480e6441d6b7e5883a84f4d8190 ;;
    guardiana_1.0.28_amd64.deb) echo b0be59f7321d79b6a2d3f15953e6f168de30e21b006bbddc35f0f85555dfadc8 ;;
    guardiana-1.0.28-windows-x86_64.exe) echo 9a0eff90769968eaf4a4de05ccd2b3d30fee8198158d5cc17bee4786633702b0 ;;
    guardiana-1.0.28-linux-x86_64.tar.gz) echo 100dba77c38ce0ea1677dc88d389324a053bb7213d3508bfebc0faa113a32907 ;;
    guardiana-1.0.28-windows-x64.msi) echo f61fcb9836bc6049ce0713874825898cf9923c36436e120b64d1186c34b1ec04 ;;
    guardiana-1.0.28-windows-x64-pt.msi) echo fc5037bae96e73434ee89abadd40e723095022a85318fca380dcd4d3492fef66 ;;
    guardiana-1.0.28-windows-x64-en.msi) echo f60e42636b3dda8ca7faec126e4061b8afe407a9f89a62a89374f80ebc00c760 ;;
  esac
}
echo "4. Comprobando"
mal=0; n=0
: > "$ROOT/firmar/SHA256SUMS"
for f in "$ROOT/firmar"/guardiana*; do
  [ -f "$f" ] || continue
  n=$((n+1)); base=$(basename "$f"); tengo=$(huella "$f")
  if [ "$base" = "guardiana-1.0.28-linux-x86_64" ]; then
    echo "   OK        $base (dentro del tar.gz)"
    printf '%s *%s\n' "$tengo" "$base" >> "$ROOT/firmar/SHA256SUMS"
    continue
  fi
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
if [ "$n" -eq 0 ] || [ "$mal" != 0 ]; then echo "5. HUELLAS MAL. No firmo."; exit 1; fi
echo "5. HUELLAS OK"
REPO="/c/Users/franc/guardiana-1028"
if [ ! -d "$REPO/.git" ]; then
  git clone --branch candidato-1.0.28 https://github.com/guardianagroup/guardiana.git "$REPO"
fi
cd "$REPO"
git fetch --depth 1 origin 5a0c7f50dc06327335839a5a4e99514f36cfe242
git checkout --detach 5a0c7f50dc06327335839a5a4e99514f36cfe242
git config user.email "francistierraplana@icloud.com"
git config user.name "guardianagroup"
echo "6. Pide la contrasena. Es la misma, y puede pedirla varias veces."
bash build/release.sh --paquetes "$ROOT/firmar" || { echo "PARA: release.sh fallo. No esta firmado."; exit 1; }
git push origin HEAD:refs/heads/publicar-1.0.28 && echo "7. FIRMA SUBIDA" || echo "7. Firmado en este PC, pero el push fallo. Copia el error."
