#!/usr/bin/env bash
# Sube la 1.0.46 ya firmada a la web. No pide la clave.
set -euo pipefail
echo "1. Preparo los archivos firmados"
SRC="/c/Users/franc/Documents/Codex/2026-10-18/files-pasted-by-the-user-guardiana/work/guardiana"
WEB="/c/Users/franc/web-guardiana"
PAQ="/c/Users/franc/Downloads/para-la-web-1046"
cd "$SRC"
rm -rf "$PAQ"
mkdir -p "$PAQ"
tomar() {
  local n="$1" f
  for f in "$SRC/dist/1.0.46/$n" "$SRC/build/out/$n" "$SRC/build/out/signed/$n"; do
    if [ -f "$f" ]; then cp -f "$f" "$PAQ/$n"; echo "   tengo $n"; return 0; fi
  done
  echo "FALTA $n"; exit 1
}
for n in \
  guardiana-1.0.46-windows-x86_64.exe guardiana-1.0.46-windows-x86_64.exe.minisig \
  guardiana-1.0.46-windows-x64.msi guardiana-1.0.46-windows-x64.msi.minisig \
  guardiana-1.0.46-windows-x64-en.msi guardiana-1.0.46-windows-x64-en.msi.minisig \
  guardiana-1.0.46-windows-x64-pt.msi guardiana-1.0.46-windows-x64-pt.msi.minisig \
  guardiana-1.0.46-macos.app.zip guardiana-1.0.46-macos.app.zip.minisig
do tomar "$n"; done
echo "2. Compruebo las huellas"
PYBIN=python
if ! "$PYBIN" -c "import hashlib" >/dev/null 2>&1; then PYBIN=python3; fi
export PAQ_WIN WEB_WIN
PAQ_WIN=$(cygpath -w "$PAQ")
"$PYBIN" - << "PY"
import hashlib, os, pathlib, sys
esp = {
 "guardiana-1.0.46-windows-x86_64.exe": "4a25bca6f9b9662e50e9c50a740e2f90bcefa25102ec73a177d2709e360cd23c",
 "guardiana-1.0.46-windows-x64.msi": "69d1e7dff8a1dca02d5f49d41275bdff46b975278f468c1af657d3d5b7c342e8",
 "guardiana-1.0.46-windows-x64-en.msi": "c9fc47422ef1534cddcb6b43010bf8c9ca97565db8d3a7a74423bc19d58cc6b1",
 "guardiana-1.0.46-windows-x64-pt.msi": "e1ba3285c4d7d890a2952733f158c452d1b01d7d9b74fd9f72e5916efd139f00",
 "guardiana-1.0.46-macos.app.zip": "cd043e356e5d330ab88f585a9fded4a6514164dca44996cc80bc5096fdd1c201",
}
base = pathlib.Path(os.environ["PAQ_WIN"])
for name, want in esp.items():
    got = hashlib.sha256((base/name).read_bytes()).hexdigest()
    if got != want:
        sys.exit("MAL " + name)
    print("   OK", name)
PY
echo "3. Bajo la web. Tarda unos minutos. No cierres la ventana."
if [ ! -d "$WEB/.git" ]; then
  git clone --depth 1 https://github.com/guardianagroup/guardianagroup.com.git "$WEB"
else
  git -C "$WEB" fetch origin main
  git -C "$WEB" checkout -B main origin/main
fi
rm -rf "$WEB/descargas/1.0.46"
mkdir -p "$WEB/descargas/1.0.46"
cp -f "$PAQ"/* "$WEB/descargas/1.0.46/"
echo "4. Actualizo las paginas"
WEB_WIN=$(cygpath -w "$WEB")
"$PYBIN" - << "PY"
import os, pathlib, re
web = pathlib.Path(os.environ["WEB_WIN"])
files = ["instalar.html","index.html","zero.html","es/index.html","en/install.html","en/zero.html","pt/instalar.html","pt/index.html","pt/zero.html"]
hashes = {
 "3315d15326dd2001ba2e837c95e00ba5886880ad4d86594b589401273335efeb": "69d1e7dff8a1dca02d5f49d41275bdff46b975278f468c1af657d3d5b7c342e8",
 "b3440b8ca2a2ccb591ee98a32b4beffb535397b5cf748f71845df306bd599602": "c9fc47422ef1534cddcb6b43010bf8c9ca97565db8d3a7a74423bc19d58cc6b1",
 "f815069927af3f15b244aaddce37388d8e4e4aae77f8ad9a81d626a20ff767b2": "e1ba3285c4d7d890a2952733f158c452d1b01d7d9b74fd9f72e5916efd139f00",
 "10b172137b128e8404d4cc764cdde3b89218886debb8d436e74e7266dd5a7e22": "cd043e356e5d330ab88f585a9fded4a6514164dca44996cc80bc5096fdd1c201",
}
sizes = {
 "guardiana-1.0.46-windows-x64.msi": ("55,7 MB", "55.7 MB"),
 "guardiana-1.0.46-windows-x64-en.msi": ("55,6 MB", "55.6 MB"),
 "guardiana-1.0.46-windows-x64-pt.msi": ("55,5 MB", "55.5 MB"),
 "guardiana-1.0.46-macos.app.zip": ("29,5 MB", "29.5 MB"),
}
for rel in files:
    p = web/rel
    t = p.read_text(encoding="utf-8").replace("1.0.45", "1.0.46")
    for a, b in hashes.items():
        t = t.replace(a, b)
    for name, (comma, dot) in sizes.items():
        t = re.sub(re.escape(name) + r'(</a><br><span class="muted">)[0-9]+,[0-9]+ MB', name + r"\1" + comma, t)
        t = re.sub(re.escape(name) + r'(</a><br><span class="muted">)[0-9]+\.[0-9]+ MB', name + r"\1" + dot, t)
    if rel == "zero.html":
        t = t.replace("57,8 MB", "55,7 MB")
    elif rel == "en/zero.html":
        t = t.replace("57.8 MB", "55.6 MB")
    elif rel == "pt/zero.html":
        t = t.replace("57,9 MB", "55,5 MB")
    if "1.0.45" in t or "3315d15326dd2001" in t:
        raise SystemExit("la pagina no quedo actualizada: " + rel)
    p.write_text(t, encoding="utf-8", newline="\n")
    print("   pagina", rel)
PY
echo "5. Subo la web"
git -C "$WEB" add descargas/1.0.46 instalar.html index.html zero.html es/index.html en/install.html en/zero.html pt/instalar.html pt/index.html pt/zero.html
git -C "$WEB" -c user.name="guardianagroup" -c user.email="325701793+guardianagroup@users.noreply.github.com" commit -m "Publish GUARDIANA 1.0.46"
git -C "$WEB" push origin HEAD:main
echo "WEB SUBIDA"
