#!/usr/bin/env bash
# Sube la 1.0.27 ya firmada. No usa Python.
set -euo pipefail
echo "1. Empiezo sin Python"
SRC="/c/Users/franc/guardiana-1027"
PAQ="/c/Users/franc/Downloads/para-la-web-1027"
WEB="/c/Users/franc/web-1027"
rm -rf "$PAQ"
mkdir -p "$PAQ"
for d in "$SRC/build/out" "$SRC/dist/1.0.27"; do
  [ -d "$d" ] || continue
  find "$d" -maxdepth 1 -type f \( -name 'guardiana-1.0.27*' -o -name 'guardiana_1.0.27*' \) -exec cp -f {} "$PAQ/" \;
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
comprobar guardiana-1.0.27-linux-x86_64 54431dfe33ac921200bd4080bfc227c6223f568bf6a7533cd857f7970cc0ded3
comprobar guardiana-1.0.27-windows-x86_64.exe 22c901703c5c137fe115eeb956cb9d28476b515cc1e5bf8b77745dcad61fc5b8
comprobar guardiana-1.0.27-windows-x64.msi e1fe0960c1187adcb5ea07a90336cf0d98bcd13b509410022efecba3bba79110
comprobar guardiana-1.0.27-windows-x64-en.msi bdfb22b1b3f0c131871fc0889f3fc4149317dae1113edc688b718b1cd1e907ac
comprobar guardiana-1.0.27-windows-x64-pt.msi 493c26ea8f95e1a30276d7c35f728b3c4c1928d28d031446b4456d4a6f1518f9
comprobar guardiana_1.0.27_amd64.deb 1b2fd0bf1bff9ad3ca3c04840b35510b2f7f76f6845a42535737352443e3165d
comprobar guardiana-1.0.27-linux-x86_64.tar.gz 1360734347ad9f5de546336feac2729fac4a9483c269aee7851d36e49cebb133
comprobar guardiana-1.0.27-macos.app.zip 3e961ae28601a2b680b3eb09b974cd6895ab33062e12e934bc5ba8f4a02c15e8
[ "$ok" = 1 ] || { echo "PARA: no subo archivos que no coinciden con la firma."; exit 1; }
echo "3. HUELLAS OK"
if [ ! -d "$WEB/.git" ]; then
  echo "4. Bajando la web (tarda, son archivos grandes)"
  git clone --depth 1 https://github.com/guardianagroup/guardianagroup.com.git "$WEB"
else
  echo "4. La web ya esta bajada"
  git -C "$WEB" pull --ff-only origin main
fi
rm -rf "$WEB/descargas/1.0.27"
mkdir -p "$WEB/descargas/1.0.27"
cp -f "$PAQ"/* "$WEB/descargas/1.0.27/"
tam() { awk -v n="$(stat -c %s "$1")" -v c="$2" 'BEGIN{s=sprintf("%.1f", n/1048576); if(c=="c") gsub(/\./,",",s); printf "%s MB", s}'; }
ES=$(tam "$PAQ/guardiana-1.0.27-windows-x64.msi" c)
EN=$(tam "$PAQ/guardiana-1.0.27-windows-x64-en.msi" d)
PT=$(tam "$PAQ/guardiana-1.0.27-windows-x64-pt.msi" c)
EN_D=$(tam "$PAQ/guardiana-1.0.27-windows-x64.msi" d)
PT_D=$(tam "$PAQ/guardiana-1.0.27-windows-x64-pt.msi" d)
EN_C=$(tam "$PAQ/guardiana-1.0.27-windows-x64-en.msi" c)
DEB_C=$(tam "$PAQ/guardiana_1.0.27_amd64.deb" c)
DEB_D=$(tam "$PAQ/guardiana_1.0.27_amd64.deb" d)
TAR_C=$(tam "$PAQ/guardiana-1.0.27-linux-x86_64.tar.gz" c)
TAR_D=$(tam "$PAQ/guardiana-1.0.27-linux-x86_64.tar.gz" d)
MAC_C=$(tam "$PAQ/guardiana-1.0.27-macos.app.zip" c)
MAC_D=$(tam "$PAQ/guardiana-1.0.27-macos.app.zip" d)
for rel in index.html es/index.html pt/index.html instalar.html en/install.html pt/instalar.html zero.html en/zero.html pt/zero.html; do
  sed -i \
    -e "s/3c2319bb01f5edaf98bd5ce1a5d9a51a09ff8f0fb45e3011a42cfe555132bf6f/bdfb22b1b3f0c131871fc0889f3fc4149317dae1113edc688b718b1cd1e907ac/g" \
    -e "s/c7307d9663c072f8eef306147d20a404e50919feddbc3e8cb1e52f84fc786359/e1fe0960c1187adcb5ea07a90336cf0d98bcd13b509410022efecba3bba79110/g" \
    -e "s/2239c426d37463200456c4fb2a5d4940020f26cf6bb671240c25b0e4aef38e5e/493c26ea8f95e1a30276d7c35f728b3c4c1928d28d031446b4456d4a6f1518f9/g" \
    -e "s/1b94ea22fd227c73c8eb119b0c650e24964564df3afb463a1ceaae5d8124c33e/1b2fd0bf1bff9ad3ca3c04840b35510b2f7f76f6845a42535737352443e3165d/g" \
    -e "s/a789404646dc0a0bde2d47b5e11e5bd44994aa811ff5ea2295cacbdead69fd7d/1360734347ad9f5de546336feac2729fac4a9483c269aee7851d36e49cebb133/g" \
    -e "s/b941255f97d4e654b4b33092d3248cf57387c376e6b39f9eeb6e1a0b72062f64/3e961ae28601a2b680b3eb09b974cd6895ab33062e12e934bc5ba8f4a02c15e8/g" \
    -e "s/108e9186e8c5677a2e50d18aeac9a2c8af21b7c140fa4e542e80fedd0dedad50b3b0d78a31fa17a2/108e9186e8c5677ab553c663014824a5594930ce89d60463e5b61ef651a3a89c49abdc4c0d6cc689/g" \
    -e "s/108e9186e8c5677a…31fa17a2/108e9186e8c5677a…0d6cc689/g" \
    -e "s/1\\.0\\.26/1.0.27/g" \
    "$WEB/$rel"
  echo "   pagina $rel"
done
for rel in instalar.html pt/instalar.html; do
  sed -i -E \
    -e "s|(guardiana-1.0.27-windows-x64\\.msi</a><br><span class=\"muted\">)[0-9]+,[0-9]+ MB|\\1$ES|" \
    -e "s|(guardiana-1.0.27-windows-x64-en\\.msi</a><br><span class=\"muted\">)[0-9]+,[0-9]+ MB|\\1$EN_C|" \
    -e "s|(guardiana-1.0.27-windows-x64-pt\\.msi</a><br><span class=\"muted\">)[0-9]+,[0-9]+ MB|\\1$PT|" \
    -e "s|(guardiana_1\\.0\\.27_amd64\\.deb</a><br><span class=\"muted\">)[0-9]+,[0-9]+ MB|\\1$DEB_C|" \
    -e "s|(guardiana-1.0.27-linux-x86_64\\.tar\\.gz</a><br><span class=\"muted\">)[0-9]+,[0-9]+ MB|\\1$TAR_C|" \
    -e "s|(guardiana-1.0.27-macos\\.app\\.zip</a><br><span class=\"muted\">)[0-9]+,[0-9]+ MB|\\1$MAC_C|" \
    "$WEB/$rel"
done
sed -i -E \
  -e "s|(guardiana-1.0.27-windows-x64\\.msi</a><br><span class=\"muted\">)[0-9]+\\.[0-9]+ MB|\\1$EN_D|" \
  -e "s|(guardiana-1.0.27-windows-x64-en\\.msi</a><br><span class=\"muted\">)[0-9]+\\.[0-9]+ MB|\\1$EN|" \
  -e "s|(guardiana-1.0.27-windows-x64-pt\\.msi</a><br><span class=\"muted\">)[0-9]+\\.[0-9]+ MB|\\1$PT_D|" \
  -e "s|(guardiana_1\\.0\\.27_amd64\\.deb</a><br><span class=\"muted\">)[0-9]+\\.[0-9]+ MB|\\1$DEB_D|" \
  -e "s|(guardiana-1.0.27-linux-x86_64\\.tar\\.gz</a><br><span class=\"muted\">)[0-9]+\\.[0-9]+ MB|\\1$TAR_D|" \
  -e "s|(guardiana-1.0.27-macos\\.app\\.zip</a><br><span class=\"muted\">)[0-9]+\\.[0-9]+ MB|\\1$MAC_D|" \
  "$WEB/en/install.html"
sed -i -E "s/Windows 10 y 11 · [0-9]+,[0-9]+ MB/Windows 10 y 11 · $ES/; s/español · [0-9]+,[0-9]+ MB/español · $ES/" "$WEB/zero.html"
sed -i -E "s/Windows 10 and 11 · [0-9]+\\.[0-9]+ MB/Windows 10 and 11 · $EN/; s/English · [0-9]+\\.[0-9]+ MB/English · $EN/" "$WEB/en/zero.html"
sed -i -E "s/Windows 10 e 11 · [0-9]+,[0-9]+ MB/Windows 10 e 11 · $PT/; s/português · [0-9]+,[0-9]+ MB/português · $PT/" "$WEB/pt/zero.html"
echo "5. Paginas actualizadas"
cd "$WEB"
git config user.email "francistierraplana@icloud.com"
git config user.name "guardianagroup"
git add descargas/1.0.27 index.html es/index.html pt/index.html instalar.html en/install.html pt/instalar.html zero.html en/zero.html pt/zero.html
if git diff --cached --quiet; then echo "PARA: no hay cambios que subir."; exit 1; fi
git commit -m "Publish Guardiana 1.0.27"
git push origin HEAD:refs/heads/main
echo "6. WEB SUBIDA"
