#!/usr/bin/env bash
# Sube la 1.0.30 ya firmada. No usa Python.
set -euo pipefail
echo "1. Empiezo a subir la 1.0.30 a la web"
SRC="/c/Users/franc/guardiana-1030"
PAQ="/c/Users/franc/Downloads/para-la-web-1030"
WEB="/c/Users/franc/web-1027"
rm -rf "$PAQ"
mkdir -p "$PAQ"
for d in "$SRC/build/out" "$SRC/build/out/signed" "$SRC/dist/1.0.30"; do
  [ -d "$d" ] || continue
  find "$d" -maxdepth 1 -type f \( -name 'guardiana-1.0.30*' -o -name 'guardiana_1.0.30*' \) -exec cp -f {} "$PAQ/" \;
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
comprobar guardiana-1.0.30-linux-x86_64 abbc6c7718556417d5e8a35c717c995e8077fa4b4e20622e068dd0acdac8dd8c
comprobar guardiana-1.0.30-windows-x86_64.exe 987b8944e584add9eab2ada6860ce2cab9c5265e814f09e4faf2835343c94b4b
comprobar guardiana-1.0.30-windows-x64.msi 11d19206a6b459f6c19a9a0be6ad01865e793816a7d1015c4a9db549c45294e1
comprobar guardiana-1.0.30-windows-x64-en.msi eaddb93e329a5eb6be89ad38ae9a01a13593c5feec4ba99eaa54f596e66dfc52
comprobar guardiana-1.0.30-windows-x64-pt.msi 7842ca0995b662b5f28ef226b8bdb68a386a5c76af00571fe48de442764ae21e
comprobar guardiana_1.0.30_amd64.deb fb4a4ce28c88c4b51ccebb39802a513f958fb5192c5619d80719f4d16039b8c8
comprobar guardiana-1.0.30-linux-x86_64.tar.gz 26a715f999afe5baaae6dd97b61345b5a3677e980eb6e664cb008b22b09630ff
comprobar guardiana-1.0.30-macos.app.zip 30c597a6eb3e1b46ae0992b5a53848b069169ccaa498e76b9a50601cef1d5bed
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
rm -rf "$WEB/descargas/1.0.30"
mkdir -p "$WEB/descargas/1.0.30"
cp -f "$PAQ"/* "$WEB/descargas/1.0.30/"
tam() { awk -v n="$(stat -c %s "$1")" -v c="$2" 'BEGIN{s=sprintf("%.1f", n/1048576); if(c=="c") gsub(/\./,",",s); printf "%s MB", s}'; }
ES=$(tam "$PAQ/guardiana-1.0.30-windows-x64.msi" c)
EN=$(tam "$PAQ/guardiana-1.0.30-windows-x64-en.msi" d)
PT=$(tam "$PAQ/guardiana-1.0.30-windows-x64-pt.msi" c)
EN_D=$(tam "$PAQ/guardiana-1.0.30-windows-x64.msi" d)
PT_D=$(tam "$PAQ/guardiana-1.0.30-windows-x64-pt.msi" d)
EN_C=$(tam "$PAQ/guardiana-1.0.30-windows-x64-en.msi" c)
DEB_C=$(tam "$PAQ/guardiana_1.0.30_amd64.deb" c)
DEB_D=$(tam "$PAQ/guardiana_1.0.30_amd64.deb" d)
TAR_C=$(tam "$PAQ/guardiana-1.0.30-linux-x86_64.tar.gz" c)
TAR_D=$(tam "$PAQ/guardiana-1.0.30-linux-x86_64.tar.gz" d)
MAC_C=$(tam "$PAQ/guardiana-1.0.30-macos.app.zip" c)
MAC_D=$(tam "$PAQ/guardiana-1.0.30-macos.app.zip" d)
for rel in index.html es/index.html pt/index.html instalar.html en/install.html pt/instalar.html zero.html en/zero.html pt/zero.html; do
  sed -i \
    -e "s/f60e42636b3dda8ca7faec126e4061b8afe407a9f89a62a89374f80ebc00c760/eaddb93e329a5eb6be89ad38ae9a01a13593c5feec4ba99eaa54f596e66dfc52/g" \
    -e "s/f61fcb9836bc6049ce0713874825898cf9923c36436e120b64d1186c34b1ec04/11d19206a6b459f6c19a9a0be6ad01865e793816a7d1015c4a9db549c45294e1/g" \
    -e "s/fc5037bae96e73434ee89abadd40e723095022a85318fca380dcd4d3492fef66/7842ca0995b662b5f28ef226b8bdb68a386a5c76af00571fe48de442764ae21e/g" \
    -e "s/b0be59f7321d79b6a2d3f15953e6f168de30e21b006bbddc35f0f85555dfadc8/fb4a4ce28c88c4b51ccebb39802a513f958fb5192c5619d80719f4d16039b8c8/g" \
    -e "s/100dba77c38ce0ea1677dc88d389324a053bb7213d3508bfebc0faa113a32907/26a715f999afe5baaae6dd97b61345b5a3677e980eb6e664cb008b22b09630ff/g" \
    -e "s/11adb67558c953570cb576633f05b6a06db68480e6441d6b7e5883a84f4d8190/30c597a6eb3e1b46ae0992b5a53848b069169ccaa498e76b9a50601cef1d5bed/g" \
    -e "s/108e9186e8c5677a67a6f7c7a432b7590ec2e0249d789fb62f5d9d1e22ea4b517471a002fff43b5e/108e9186e8c5677a64b287effb1c30a961b312be493639e35808b66b03b34dc0a2aba6ac9540861c/g" \
    -e "s/108e9186e8c5677a…fff43b5e/108e9186e8c5677a…9540861c/g" \
    -e "s/1\\.0\\.28/1.0.30/g" \
    "$WEB/$rel"
  echo "   pagina $rel"
done
grep -q '11d19206a6b459f6c19a9a0be6ad01865e793816a7d1015c4a9db549c45294e1' "$WEB/instalar.html" || { echo "PARA: la pagina en espanol no quedo con la huella nueva."; exit 1; }
for rel in instalar.html pt/instalar.html; do
  sed -i -E \
    -e "s|(guardiana-1.0.30-windows-x64\\.msi</a><br><span class=\"muted\">)[0-9]+,[0-9]+ MB|\\1$ES|" \
    -e "s|(guardiana-1.0.30-windows-x64-en\\.msi</a><br><span class=\"muted\">)[0-9]+,[0-9]+ MB|\\1$EN_C|" \
    -e "s|(guardiana-1.0.30-windows-x64-pt\\.msi</a><br><span class=\"muted\">)[0-9]+,[0-9]+ MB|\\1$PT|" \
    -e "s|(guardiana_1\\.0\\.30_amd64\\.deb</a><br><span class=\"muted\">)[0-9]+,[0-9]+ MB|\\1$DEB_C|" \
    -e "s|(guardiana-1.0.30-linux-x86_64\\.tar\\.gz</a><br><span class=\"muted\">)[0-9]+,[0-9]+ MB|\\1$TAR_C|" \
    -e "s|(guardiana-1.0.30-macos\\.app\\.zip</a><br><span class=\"muted\">)[0-9]+,[0-9]+ MB|\\1$MAC_C|" \
    "$WEB/$rel"
done
sed -i -E \
  -e "s|(guardiana-1.0.30-windows-x64\\.msi</a><br><span class=\"muted\">)[0-9]+\\.[0-9]+ MB|\\1$EN_D|" \
  -e "s|(guardiana-1.0.30-windows-x64-en\\.msi</a><br><span class=\"muted\">)[0-9]+\\.[0-9]+ MB|\\1$EN|" \
  -e "s|(guardiana-1.0.30-windows-x64-pt\\.msi</a><br><span class=\"muted\">)[0-9]+\\.[0-9]+ MB|\\1$PT_D|" \
  -e "s|(guardiana_1\\.0\\.30_amd64\\.deb</a><br><span class=\"muted\">)[0-9]+\\.[0-9]+ MB|\\1$DEB_D|" \
  -e "s|(guardiana-1.0.30-linux-x86_64\\.tar\\.gz</a><br><span class=\"muted\">)[0-9]+\\.[0-9]+ MB|\\1$TAR_D|" \
  -e "s|(guardiana-1.0.30-macos\\.app\\.zip</a><br><span class=\"muted\">)[0-9]+\\.[0-9]+ MB|\\1$MAC_D|" \
  "$WEB/en/install.html"
sed -i -E "s/Windows 10 y 11 · [0-9]+,[0-9]+ MB/Windows 10 y 11 · $ES/; s/español · [0-9]+,[0-9]+ MB/español · $ES/" "$WEB/zero.html"
sed -i -E "s/Windows 10 and 11 · [0-9]+\\.[0-9]+ MB/Windows 10 and 11 · $EN/; s/English · [0-9]+\\.[0-9]+ MB/English · $EN/" "$WEB/en/zero.html"
sed -i -E "s/Windows 10 e 11 · [0-9]+,[0-9]+ MB/Windows 10 e 11 · $PT/; s/português · [0-9]+,[0-9]+ MB/português · $PT/" "$WEB/pt/zero.html"
echo "5. Paginas actualizadas"
cd "$WEB"
git config user.email "francistierraplana@icloud.com"
git config user.name "guardianagroup"
git add descargas/1.0.30 index.html es/index.html pt/index.html instalar.html en/install.html pt/instalar.html zero.html en/zero.html pt/zero.html
if git diff --cached --quiet; then echo "PARA: no hay cambios que subir."; exit 1; fi
git commit -m "Publish Guardiana 1.0.30"
git push origin HEAD:refs/heads/main
echo "6. WEB SUBIDA"
