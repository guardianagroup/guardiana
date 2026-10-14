#!/usr/bin/env bash
# Copia la 1.0.27 ya firmada a la web y publica guardianagroup.com.
set -euo pipefail
echo "1. Empiezo a subir la 1.0.27 a la web"
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
python3 - "$PAQ" << 'PY'
import hashlib, sys
from pathlib import Path
paq = Path(sys.argv[1])
want = {
"guardiana-1.0.27-linux-x86_64": "54431dfe33ac921200bd4080bfc227c6223f568bf6a7533cd857f7970cc0ded3",
"guardiana-1.0.27-windows-x86_64.exe": "22c901703c5c137fe115eeb956cb9d28476b515cc1e5bf8b77745dcad61fc5b8",
"guardiana-1.0.27-windows-x64.msi": "e1fe0960c1187adcb5ea07a90336cf0d98bcd13b509410022efecba3bba79110",
"guardiana-1.0.27-windows-x64-en.msi": "bdfb22b1b3f0c131871fc0889f3fc4149317dae1113edc688b718b1cd1e907ac",
"guardiana-1.0.27-windows-x64-pt.msi": "493c26ea8f95e1a30276d7c35f728b3c4c1928d28d031446b4456d4a6f1518f9",
"guardiana_1.0.27_amd64.deb": "1b2fd0bf1bff9ad3ca3c04840b35510b2f7f76f6845a42535737352443e3165d",
"guardiana-1.0.27-linux-x86_64.tar.gz": "1360734347ad9f5de546336feac2729fac4a9483c269aee7851d36e49cebb133",
"guardiana-1.0.27-macos.app.zip": "3e961ae28601a2b680b3eb09b974cd6895ab33062e12e934bc5ba8f4a02c15e8",
}
bad = 0
for name, exp in want.items():
    f, s = paq / name, paq / (name + ".minisig")
    if not f.is_file() or not s.is_file():
        print("   FALTA", name if not f.is_file() else name + ".minisig")
        bad = 1
        continue
    h = hashlib.sha256(f.read_bytes()).hexdigest()
    if h != exp:
        print("   MAL  ", name)
        bad = 1
    else:
        print("   OK   ", name)
if bad:
    sys.exit("PARA: no subo archivos que no coinciden con la firma.")
print("3. HUELLAS OK")
PY
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
python3 - "$WEB" "$PAQ" << 'PY'
import re, sys
from pathlib import Path
web, paq = Path(sys.argv[1]), Path(sys.argv[2])
pages = [
"index.html","es/index.html","pt/index.html",
"instalar.html","en/install.html","pt/instalar.html",
"zero.html","en/zero.html","pt/zero.html",
]
hashes = {
"3c2319bb01f5edaf98bd5ce1a5d9a51a09ff8f0fb45e3011a42cfe555132bf6f":"bdfb22b1b3f0c131871fc0889f3fc4149317dae1113edc688b718b1cd1e907ac",
"c7307d9663c072f8eef306147d20a404e50919feddbc3e8cb1e52f84fc786359":"e1fe0960c1187adcb5ea07a90336cf0d98bcd13b509410022efecba3bba79110",
"2239c426d37463200456c4fb2a5d4940020f26cf6bb671240c25b0e4aef38e5e":"493c26ea8f95e1a30276d7c35f728b3c4c1928d28d031446b4456d4a6f1518f9",
"1b94ea22fd227c73c8eb119b0c650e24964564df3afb463a1ceaae5d8124c33e":"1b2fd0bf1bff9ad3ca3c04840b35510b2f7f76f6845a42535737352443e3165d",
"a789404646dc0a0bde2d47b5e11e5bd44994aa811ff5ea2295cacbdead69fd7d":"1360734347ad9f5de546336feac2729fac4a9483c269aee7851d36e49cebb133",
"b941255f97d4e654b4b33092d3248cf57387c376e6b39f9eeb6e1a0b72062f64":"3e961ae28601a2b680b3eb09b974cd6895ab33062e12e934bc5ba8f4a02c15e8",
}
old_rekor = "108e9186e8c5677a2e50d18aeac9a2c8af21b7c140fa4e542e80fedd0dedad50b3b0d78a31fa17a2"
new_rekor = "108e9186e8c5677ab553c663014824a5594930ce89d60463e5b61ef651a3a89c49abdc4c0d6cc689"
sizes = {p.name: p.stat().st_size for p in paq.iterdir() if p.is_file() and not p.name.endswith(".minisig")}
pat = re.compile(r"(\d+[\.,]\d+)\s*MB")
def mb(n, comma):
    s = f"{n/1048576:.1f}"
    return (s.replace(".", ",") if comma else s) + " MB"
for rel in pages:
    path = web / rel
    text = path.read_text(encoding="utf-8")
    for old, new in hashes.items():
        text = text.replace(old, new)
    text = text.replace(old_rekor, new_rekor)
    text = text.replace("108e9186e8c5677a\u202631fa17a2", "108e9186e8c5677a\u20260d6cc689")
    text = text.replace("1.0.26", "1.0.27")
    for name, nbytes in sizes.items():
        start = 0
        while True:
            i = text.find(name, start)
            if i < 0:
                break
            window = text[i:i+500]
            m = pat.search(window)
            if m and "MB" in window[m.start():m.end()]:
                comma = "," in m.group(1)
                nuevo = mb(nbytes, comma)
                text = text[:i+m.start()] + nuevo + text[i+m.end():]
                start = i + m.start() + len(nuevo)
            else:
                start = i + len(name)
    if "1.0.26" in text or old_rekor in text or any(h in text for h in hashes):
        sys.exit("PARA: quedo una huella vieja en " + rel)
    path.write_text(text, encoding="utf-8", newline="\n")
    print("   pagina", rel)
print("5. Paginas actualizadas")
PY
cd "$WEB"
git config user.email "francistierraplana@icloud.com"
git config user.name "guardianagroup"
git add descargas/1.0.27 index.html es/index.html pt/index.html instalar.html en/install.html pt/instalar.html zero.html en/zero.html pt/zero.html
if git diff --cached --quiet; then
  echo "PARA: no hay cambios que subir."
  exit 1
fi
git commit -m "Publish Guardiana 1.0.27"
git push origin HEAD:refs/heads/main
echo "6. WEB SUBIDA"
