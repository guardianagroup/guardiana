#!/usr/bin/env bash
# Sube la 1.0.28 ya firmada. No usa Python.
set -euo pipefail
echo "1. Empiezo a subir la 1.0.28 a la web"
SRC="/c/Users/franc/guardiana-1028"
PAQ="/c/Users/franc/Downloads/para-la-web-1028"
WEB="/c/Users/franc/web-1027"
rm -rf "$PAQ"
mkdir -p "$PAQ"
for d in "$SRC/build/out" "$SRC/dist/1.0.28"; do
  [ -d "$d" ] || continue
  find "$d" -maxdepth 1 -type f \( -name 'guardiana-1.0.28*' -o -name 'guardiana_1.0.28*' \) -exec cp -f {} "$PAQ/" \;
done
echo "2. Archivos:"
ls -1 "$PAQ"
huella() { openssl dgst -sha256 "$1" | awk '{print tolower($NF)}'; }
ok=1
comprobar() {
  local f="$PAQ/$1" tengo
  if [ ! -f "$f" ] || [ ! -f "$f.minisig" ]; then echo "   FALTA $1"; ok=0; return; fi
  tengo=$(huella "$f")
  if [ "$tengo" = "$2" ]; then echo "   OK    $1"; else echo "   MAL   $1"; echo "         $tengo"; ok=0; fi
}
comprobar guardiana-1.0.28-linux-x86_64 4d8757fc04a5282e31476794f8bc1b0622956a5706bf4e35c4a122d104a67206
comprobar guardiana-1.0.28-windows-x86_64.exe 9a0eff90769968eaf4a4de05ccd2b3d30fee8198158d5cc17bee4786633702b0
comprobar guardiana-1.0.28-windows-x64.msi f61fcb9836bc6049ce0713874825898cf9923c36436e120b64d1186c34b1ec04
comprobar guardiana-1.0.28-windows-x64-en.msi f60e42636b3dda8ca7faec126e4061b8afe407a9f89a62a89374f80ebc00c760
comprobar guardiana-1.0.28-windows-x64-pt.msi fc5037bae96e73434ee89abadd40e723095022a85318fca380dcd4d3492fef66
comprobar guardiana_1.0.28_amd64.deb b0be59f7321d79b6a2d3f15953e6f168de30e21b006bbddc35f0f85555dfadc8
comprobar guardiana-1.0.28-linux-x86_64.tar.gz 100dba77c38ce0ea1677dc88d389324a053bb7213d3508bfebc0faa113a32907
comprobar guardiana-1.0.28-macos.app.zip 11adb67558c953570cb576633f05b6a06db68480e6441d6b7e5883a84f4d8190
[ "$ok" = 1 ] || { echo "PARA: no subo archivos que no coinciden con la firma."; exit 1; }
echo "3. HUELLAS OK"
if [ ! -d "$WEB/.git" ]; then
  echo "4. Bajando la web (tarda, son archivos grandes)"
  git clone --depth 1 https://github.com/guardianagroup/guardianagroup.com.git "$WEB"
else
  echo "4. Actualizo la web que ya esta en el PC"
  git -C "$WEB" fetch origin main
  git -C "$WEB" checkout -B main origin/main
fi
rm -rf "$WEB/descargas/1.0.28"
mkdir -p "$WEB/descargas/1.0.28"
cp -f "$PAQ"/* "$WEB/descargas/1.0.28/"
tam() { awk -v n="$(stat -c %s "$1")" -v c="$2" 'BEGIN{s=sprintf("%.1f", n/1048576); if(c=="c") gsub(/\./,",",s); printf "%s MB", s}'; }
ES=$(tam "$PAQ/guardiana-1.0.28-windows-x64.msi" c)
EN=$(tam "$PAQ/guardiana-1.0.28-windows-x64-en.msi" d)
PT=$(tam "$PAQ/guardiana-1.0.28-windows-x64-pt.msi" c)
EN_D=$(tam "$PAQ/guardiana-1.0.28-windows-x64.msi" d)
PT_D=$(tam "$PAQ/guardiana-1.0.28-windows-x64-pt.msi" d)
EN_C=$(tam "$PAQ/guardiana-1.0.28-windows-x64-en.msi" c)
DEB_C=$(tam "$PAQ/guardiana_1.0.28_amd64.deb" c)
DEB_D=$(tam "$PAQ/guardiana_1.0.28_amd64.deb" d)
TAR_C=$(tam "$PAQ/guardiana-1.0.28-linux-x86_64.tar.gz" c)
TAR_D=$(tam "$PAQ/guardiana-1.0.28-linux-x86_64.tar.gz" d)
MAC_C=$(tam "$PAQ/guardiana-1.0.28-macos.app.zip" c)
MAC_D=$(tam "$PAQ/guardiana-1.0.28-macos.app.zip" d)
for rel in index.html es/index.html pt/index.html instalar.html en/install.html pt/instalar.html zero.html en/zero.html pt/zero.html; do
  sed -i \
    -e "s/bdfb22b1b3f0c131871fc0889f3fc4149317dae1113edc688b718b1cd1e907ac/f60e42636b3dda8ca7faec126e4061b8afe407a9f89a62a89374f80ebc00c760/g" \
    -e "s/e1fe0960c1187adcb5ea07a90336cf0d98bcd13b509410022efecba3bba79110/f61fcb9836bc6049ce0713874825898cf9923c36436e120b64d1186c34b1ec04/g" \
    -e "s/493c26ea8f95e1a30276d7c35f728b3c4c1928d28d031446b4456d4a6f1518f9/fc5037bae96e73434ee89abadd40e723095022a85318fca380dcd4d3492fef66/g" \
    -e "s/1b2fd0bf1bff9ad3ca3c04840b35510b2f7f76f6845a42535737352443e3165d/b0be59f7321d79b6a2d3f15953e6f168de30e21b006bbddc35f0f85555dfadc8/g" \
    -e "s/1360734347ad9f5de546336feac2729fac4a9483c269aee7851d36e49cebb133/100dba77c38ce0ea1677dc88d389324a053bb7213d3508bfebc0faa113a32907/g" \
    -e "s/3e961ae28601a2b680b3eb09b974cd6895ab33062e12e934bc5ba8f4a02c15e8/11adb67558c953570cb576633f05b6a06db68480e6441d6b7e5883a84f4d8190/g" \
    -e "s/108e9186e8c5677ab553c663014824a5594930ce89d60463e5b61ef651a3a89c49abdc4c0d6cc689/108e9186e8c5677a67a6f7c7a432b7590ec2e0249d789fb62f5d9d1e22ea4b517471a002fff43b5e/g" \
    -e "s/108e9186e8c5677a…0d6cc689/108e9186e8c5677a…fff43b5e/g" \
    -e "s/1\\.0\\.27/1.0.28/g" \
    "$WEB/$rel"
  echo "   pagina $rel"
done
for rel in instalar.html pt/instalar.html; do
  sed -i -E \
    -e "s|(guardiana-1.0.28-windows-x64\\.msi</a><br><span class=\"muted\">)[0-9]+,[0-9]+ MB|\\1$ES|" \
    -e "s|(guardiana-1.0.28-windows-x64-en\\.msi</a><br><span class=\"muted\">)[0-9]+,[0-9]+ MB|\\1$EN_C|" \
    -e "s|(guardiana-1.0.28-windows-x64-pt\\.msi</a><br><span class=\"muted\">)[0-9]+,[0-9]+ MB|\\1$PT|" \
    -e "s|(guardiana_1\\.0\\.28_amd64\\.deb</a><br><span class=\"muted\">)[0-9]+,[0-9]+ MB|\\1$DEB_C|" \
    -e "s|(guardiana-1.0.28-linux-x86_64\\.tar\\.gz</a><br><span class=\"muted\">)[0-9]+,[0-9]+ MB|\\1$TAR_C|" \
    -e "s|(guardiana-1.0.28-macos\\.app\\.zip</a><br><span class=\"muted\">)[0-9]+,[0-9]+ MB|\\1$MAC_C|" \
    "$WEB/$rel"
done
sed -i -E \
  -e "s|(guardiana-1.0.28-windows-x64\\.msi</a><br><span class=\"muted\">)[0-9]+\\.[0-9]+ MB|\\1$EN_D|" \
  -e "s|(guardiana-1.0.28-windows-x64-en\\.msi</a><br><span class=\"muted\">)[0-9]+\\.[0-9]+ MB|\\1$EN|" \
  -e "s|(guardiana-1.0.28-windows-x64-pt\\.msi</a><br><span class=\"muted\">)[0-9]+\\.[0-9]+ MB|\\1$PT_D|" \
  -e "s|(guardiana_1\\.0\\.28_amd64\\.deb</a><br><span class=\"muted\">)[0-9]+\\.[0-9]+ MB|\\1$DEB_D|" \
  -e "s|(guardiana-1.0.28-linux-x86_64\\.tar\\.gz</a><br><span class=\"muted\">)[0-9]+\\.[0-9]+ MB|\\1$TAR_D|" \
  -e "s|(guardiana-1.0.28-macos\\.app\\.zip</a><br><span class=\"muted\">)[0-9]+\\.[0-9]+ MB|\\1$MAC_D|" \
  "$WEB/en/install.html"
sed -i -E "s/Windows 10 y 11 · [0-9]+,[0-9]+ MB/Windows 10 y 11 · $ES/; s/español · [0-9]+,[0-9]+ MB/español · $ES/" "$WEB/zero.html"
sed -i -E "s/Windows 10 and 11 · [0-9]+\\.[0-9]+ MB/Windows 10 and 11 · $EN/; s/English · [0-9]+\\.[0-9]+ MB/English · $EN/" "$WEB/en/zero.html"
sed -i -E "s/Windows 10 e 11 · [0-9]+,[0-9]+ MB/Windows 10 e 11 · $PT/; s/português · [0-9]+,[0-9]+ MB/português · $PT/" "$WEB/pt/zero.html"
echo "5. Paginas actualizadas"
cd "$WEB"
git config user.email "francistierraplana@icloud.com"
git config user.name "guardianagroup"
git add descargas/1.0.28 index.html es/index.html pt/index.html instalar.html en/install.html pt/instalar.html zero.html en/zero.html pt/zero.html
if git diff --cached --quiet; then echo "PARA: no hay cambios que subir."; exit 1; fi
git commit -m "Publish Guardiana 1.0.28"
git push origin HEAD:refs/heads/main
echo "6. WEB SUBIDA"
