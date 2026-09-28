#!/bin/bash
# GUARDIANA 1.0.0 · firma, registro público y tabla de descargas, en el Mac de publicación.
#
# Lo único que se escribe a mano es la contraseña de la clave de firma: rsign/minisign la pide
# una vez por archivo (unas diez veces). La clave privada no sale de ~/guardiana-claves.
#
# Antes: los dos .zip de la ejecución 36447056747 de GitHub en ~/Downloads
# (guardiana-repro y guardiana-instaladores; si Safari los descomprimió, las carpetas valen igual).
#
# Uso:  bash publicar-1.0.0.sh
#
# Qué hace, en este orden, y dónde para si algo no cuadra:
#   0. herramientas (instala rsign y cargo-deb si faltan)
#   1. clona el código publicado en GitHub y exige que main sea exactamente COMMIT
#   2. comprueba las huellas de lo que compiló GitHub
#   3. firma un papel de prueba y exige que la clave de este Mac sea la que publica la web
#   4. build/release.sh: .deb, .tar.gz y app de Mac; firmas; línea del registro; Rekor; commit
#   5. comprueba cada firma con la clave pública
#   6. sube el registro (ledger.jsonl) a main
#   7. monta la tabla de descargas en la rama lanzamiento-1.0.0 de la web y la sube
#      (la web publicada no cambia: eso lo hace Claude después de revisarla)
set -euo pipefail

VERSION=1.0.0
COMMIT=a24bc90b2cffe17dada2c8d7eb80f89a62ff0da3
RUN=36447056747
CODIGO=https://github.com/guardianagroup/guardiana.git
WEB=https://github.com/guardianagroup/guardianagroup.com.git
RAMA_WEB=lanzamiento-1.0.0
REPO="${REPO:-$HOME/guardiana}"
DESCARGAS="${DESCARGAS:-$HOME/Downloads}"
KEYS="${GUARDIANA_KEYS:-$HOME/guardiana-claves}"
TRABAJO="${TRABAJO:-$HOME/guardiana-publicar-$VERSION}"
# Solo para el ensayo en GitHub Actions: clave de usar y tirar, Rekor de mentira y nada se sube.
ENSAYO="${ENSAYO:-}"

# Huellas de lo que compiló GitHub (ejecución 36447056747), comprobadas en el PC de pruebas.
ESPERADAS=(
  "65e7bf528bf50b5ad235f7ba084d7a52b8b6897884a9bbd610d07700bed9a33d  guardiana-1.0.0-windows-x86_64.exe"
  "8b13b586e094e845ba3b0add374aa5fb9661d7436b7f035b4b6d8c1876056549  guardiana-1.0.0-linux-x86_64"
  "399187722f75118a85af98df60a4a664dc4153c86315ba524dc67d9129494fdd  guardiana-1.0.0-windows-x64.msi"
  "280c5288fae22dabfa2f36bd564f756629e3a44c4fbfaf97d4bbb2d31aafd84b  guardiana-1.0.0-windows-x64-en.msi"
  "193237858094804c0547737732b30e636308fb050549f633a34b059b89a6e5a8  guardiana-1.0.0-windows-x64-pt.msi"
)

paso() { printf '\n== %s\n' "$*"; }
para() { printf '\nPARO: %s\n' "$*" >&2; exit 1; }

export PATH="$HOME/.cargo/bin:$PATH"

paso "0/7 Herramientas"
for c in git python3 unzip zip cargo; do
  command -v "$c" >/dev/null 2>&1 || para "falta $c en este Mac."
done
if ! command -v minisign >/dev/null 2>&1 && ! command -v rsign >/dev/null 2>&1; then
  echo "Instalo rsign (firma en el formato de minisign)…"
  cargo install rsign2 --locked
fi
if ! cargo deb --version >/dev/null 2>&1; then
  echo "Instalo cargo-deb…"
  cargo install cargo-deb --locked
fi
if [ -z "$ENSAYO" ]; then
  [ -f "$KEYS/minisign.key" ] || para "no está $KEYS/minisign.key."
fi

# Los dos artefactos de GitHub: .zip, o carpeta si Safari ya los descomprimió.
if [ -e "$TRABAJO" ]; then
  mv "$TRABAJO" "$TRABAJO.anterior-$(date +%Y%m%d%H%M%S)"
fi
mkdir -p "$TRABAJO/zip" "$TRABAJO/todo" "$TRABAJO/archivos" "$TRABAJO/bin"
fuente() {
  if [ -f "$DESCARGAS/$1.zip" ]; then
    unzip -q -o "$DESCARGAS/$1.zip" -d "$TRABAJO/zip/$1"
    echo "$TRABAJO/zip/$1"
  elif [ -d "$DESCARGAS/$1" ]; then
    echo "$DESCARGAS/$1"
  else
    return 1
  fi
}
if ! repro="$(fuente guardiana-repro)" || ! instaladores="$(fuente guardiana-instaladores)"; then
  command -v open >/dev/null 2>&1 && open "https://github.com/guardianagroup/guardiana/actions/runs/$RUN" || true
  para "faltan en $DESCARGAS guardiana-repro y/o guardiana-instaladores. Bájalos de la página que se acaba de abrir (abajo, «Artifacts») y vuelve a ejecutar esto."
fi

# macOS no siempre trae sha256sum; release.sh lo usa. shasum hace lo mismo.
if ! command -v sha256sum >/dev/null 2>&1; then
  printf '#!/bin/sh\nexec shasum -a 256 "$@"\n' > "$TRABAJO/bin/sha256sum"
  chmod +x "$TRABAJO/bin/sha256sum"
fi
export PATH="$TRABAJO/bin:$PATH"

identidad() {  # copia quién firma los commits desde el repositorio de siempre de este Mac
  if [ -d "$REPO/.git" ]; then
    for k in user.name user.email user.signingkey gpg.format gpg.ssh.program commit.gpgsign; do
      v="$(git -C "$REPO" config --get "$k" 2>/dev/null || true)"
      [ -n "$v" ] && git config "$k" "$v"
    done
  fi
  if [ -z "$(git config --get user.email || true)" ]; then
    git config user.name "GUARDIANA GROUP"
    git config user.email "guardianagroup@users.noreply.github.com"
  fi
  # Un commit de prueba que se deshace: si la firma del commit falla aquí, mejor ahora que
  # después de Rekor.
  local antes
  antes="$(git rev-parse HEAD)"
  if ! git commit -q --allow-empty -m "prueba (se deshace)" >/dev/null 2>&1; then
    git config commit.gpgsign false
    git commit -q --allow-empty -m "prueba (se deshace)" >/dev/null 2>&1 || para "git no puede hacer commits en $(pwd)."
  fi
  git reset -q --hard "$antes"
}

paso "1/7 Código publicado en GitHub: $COMMIT"
git clone -q "$CODIGO" "$TRABAJO/guardiana"
git clone -q --branch "$RAMA_WEB" "$WEB" "$TRABAJO/web"
cd "$TRABAJO/guardiana"
[ "$(git rev-parse HEAD)" = "$COMMIT" ] || para "main de GitHub está en $(git rev-parse HEAD), no en $COMMIT. Avisa a Claude."
# Por dónde se sube: el mismo remoto con el que este Mac ya sube (ssh o https con llavero).
subir="$CODIGO"
if [ -d "$REPO/.git" ]; then
  u="$(git -C "$REPO" remote -v | awk '$3=="(push)" && $2 ~ /guardianagroup\/guardiana(\.git)?$/ {print $2; exit}')"
  [ -n "$u" ] && subir="$u"
fi
subir_web="$(printf '%s' "$subir" | sed -E 's#guardiana(\.git)?$#guardianagroup.com.git#')"
git remote set-url --push origin "$subir"
git -C "$TRABAJO/web" remote set-url --push origin "$subir_web"
identidad
( cd "$TRABAJO/web" && identidad )
# La app de Mac lleva el icono de la web; crear-app.sh lo busca en site/. Se excluye aquí mismo
# para que el árbol siga limpio (release.sh lo exige) sin tocar nada publicado.
mkdir -p site
cp "$TRABAJO/web/icon-512.png" site/icon-512.png
echo "/site/" >> .git/info/exclude

if [ -n "$ENSAYO" ]; then
  echo "(ensayo: clave de usar y tirar y Rekor de mentira)"
  KEYS="$TRABAJO/clave-ensayo"
  mkdir -p "$KEYS"
  if command -v minisign >/dev/null 2>&1; then
    minisign -G -W -p "$KEYS/minisign.pub" -s "$KEYS/minisign.key"
  else
    rsign generate -W -p "$KEYS/minisign.pub" -s "$KEYS/minisign.key"
  fi
  cp "$KEYS/minisign.pub" build/pubkey/minisign.pub
  git commit -q -am "ensayo: clave de usar y tirar"
  printf '#!/bin/sh\ncat >/dev/null\necho "{\\"Location\\":\\"/api/v1/log/entries/ENSAYO0000000000\\"}"\n' > "$TRABAJO/bin/rekor-cli"
  chmod +x "$TRABAJO/bin/rekor-cli"
fi
export GUARDIANA_KEYS="$KEYS"

if command -v minisign >/dev/null 2>&1; then
  firmar() { minisign -Sm "$1" -s "$KEYS/minisign.key" -t "$2"; }
  comprobar() { minisign -Vm "$1" -p "$TRABAJO/guardiana/build/pubkey/minisign.pub" >/dev/null; }
else
  firmar() { rsign sign -s "$KEYS/minisign.key" -x "$1.minisig" -t "$2" "$1"; }
  comprobar() { rsign verify -p "$TRABAJO/guardiana/build/pubkey/minisign.pub" -x "$1.minisig" "$1" >/dev/null; }
fi

paso "2/7 Lo que compiló GitHub y sus huellas"
cp "$repro"/guardiana-* "$TRABAJO/todo/"
find "$instaladores" -name '*.msi' -exec cp {} "$TRABAJO/todo/" \;
( cd "$TRABAJO/todo" && printf '%s\n' "${ESPERADAS[@]}" | sha256sum -c - )
mkdir -p "dist/$VERSION" target/x86_64-unknown-linux-gnu/release target/x86_64-pc-windows-gnu/release
cp "$TRABAJO"/todo/*.msi "dist/$VERSION/"

paso "3/7 La clave de este Mac es la que publica la web (primera contraseña)"
printf 'GUARDIANA %s: prueba de la clave\n' "$VERSION" > "$TRABAJO/prueba-clave.txt"
firmar "$TRABAJO/prueba-clave.txt" "GUARDIANA $VERSION prueba de la clave"
comprobar "$TRABAJO/prueba-clave.txt" || para "la clave de $KEYS no es la de build/pubkey/minisign.pub. No se ha firmado nada más."
echo "Clave correcta."

paso "4/7 Paquetes, firmas, registro público y Rekor (una contraseña por archivo)"
build/release.sh --binarios "$repro"

paso "5/7 Compruebo cada firma con la clave pública"
linea="$(tail -n 1 ledger.jsonl)"
printf '%s\n' "$linea" > "$TRABAJO/archivos/ledger-line.json"
nombres="$(python3 -c 'import json,sys; print("\n".join(f["name"] for f in json.loads(sys.argv[1])["files"]))' "$linea")"
while IFS= read -r n; do
  f=""
  for d in build/out "dist/$VERSION"; do
    if [ -f "$d/$n" ]; then f="$d/$n"; break; fi
  done
  [ -n "$f" ] || para "no encuentro $n."
  comprobar "$f" || para "la firma de $n no cuadra con la clave pública."
  cp "$f" "$f.minisig" "$TRABAJO/archivos/"
  echo "ok  $n"
done <<< "$nombres"
mac_arch=""
app_bin="build/out/app/GUARDIANA.app/Contents/Resources/guardiana"
if [ -f "$app_bin" ]; then
  mac_arch="$(lipo -archs "$app_bin" 2>/dev/null || uname -m)"
fi

paso "6/7 Registro público a GitHub (main)"
if [ -n "$ENSAYO" ]; then
  echo "(ensayo: no se sube)"
  echo "$linea"
else
  git push origin HEAD:main || para "no he podido subir ledger.jsonl. Lo firmado está en $TRABAJO/archivos. Díselo a Claude."
fi

paso "7/7 Web: tabla de descargas en la rama $RAMA_WEB (la web publicada no cambia todavía)"
cat > "$TRABAJO/descargas.py" <<'PY'
#!/usr/bin/env python3
"""Fill the download box of the three install pages from the published ledger line.

Usage: descargas.py <site-repo> <folder-with-files> <ledger-line.json>
Copies every published file (and its .minisig) to <site>/descargas/<version>/ and replaces the
"no download yet" box in instalar.html, en/install.html and pt/instalar.html with a table:
file, size, SHA-256, signature. Nothing is invented: every figure comes from the ledger line,
and each hash is recomputed from the file before it is written.
"""
import hashlib
import html
import json
import os
import pathlib
import re
import shutil
import sys

site = pathlib.Path(sys.argv[1])
src = pathlib.Path(sys.argv[2])
line = json.loads(pathlib.Path(sys.argv[3]).read_text())
version = line["version"]
# The day comes from the ledger line itself (UTC), so the page never says a day the ledger does not.
_y, _m, _d = (int(x) for x in line["date"][:10].split("-"))
MESES = {
    "es": "enero febrero marzo abril mayo junio julio agosto septiembre octubre noviembre diciembre".split(),
    "en": "January February March April May June July August September October November December".split(),
    "pt": "janeiro fevereiro março abril maio junho julho agosto setembro outubro novembro dezembro".split(),
}
FECHA = {
    "es": f"{_d} de {MESES['es'][_m - 1]} de {_y}",
    "en": f"{_d} {MESES['en'][_m - 1]} {_y}",
    "pt": f"{_d} de {MESES['pt'][_m - 1]} de {_y}",
}
dest = site / "descargas" / version
dest.mkdir(parents=True, exist_ok=True)

files = {f["name"]: f for f in line["files"]}
for name, f in files.items():
    p = src / name
    sha = hashlib.sha256(p.read_bytes()).hexdigest()
    if sha != f["sha256_signed"]:
        sys.exit(f"{name}: hash on disk {sha} != ledger {f['sha256_signed']}")
    shutil.copy2(p, dest / name)
    shutil.copy2(src / (name + ".minisig"), dest / (name + ".minisig"))


def size(name):
    n = (dest / name).stat().st_size
    return f"{n / 1_000_000:.1f} MB"


ROWS = {
    "es": [
        ("Windows 10 y 11 · español", f"guardiana-{version}-windows-x64.msi"),
        ("Windows 10 y 11 · inglés", f"guardiana-{version}-windows-x64-en.msi"),
        ("Windows 10 y 11 · portugués", f"guardiana-{version}-windows-x64-pt.msi"),
        ("Debian, Ubuntu y derivados", f"guardiana_{version}_amd64.deb"),
        ("Otro Linux con systemd", f"guardiana-{version}-linux-x86_64.tar.gz"),
        ("macOS 13 o posterior", f"guardiana-{version}-macos.app.zip"),
    ],
    "en": [
        ("Windows 10 and 11 · English", f"guardiana-{version}-windows-x64-en.msi"),
        ("Windows 10 and 11 · Spanish", f"guardiana-{version}-windows-x64.msi"),
        ("Windows 10 and 11 · Portuguese", f"guardiana-{version}-windows-x64-pt.msi"),
        ("Debian, Ubuntu and derivatives", f"guardiana_{version}_amd64.deb"),
        ("Any other Linux with systemd", f"guardiana-{version}-linux-x86_64.tar.gz"),
        ("macOS 13 or later", f"guardiana-{version}-macos.app.zip"),
    ],
    "pt": [
        ("Windows 10 e 11 · português", f"guardiana-{version}-windows-x64-pt.msi"),
        ("Windows 10 e 11 · espanhol", f"guardiana-{version}-windows-x64.msi"),
        ("Windows 10 e 11 · inglês", f"guardiana-{version}-windows-x64-en.msi"),
        ("Debian, Ubuntu e derivados", f"guardiana_{version}_amd64.deb"),
        ("Outro Linux com systemd", f"guardiana-{version}-linux-x86_64.tar.gz"),
        ("macOS 13 ou posterior", f"guardiana-{version}-macos.app.zip"),
    ],
}
TEXT = {
    "es": dict(id="descargar", head=("Sistema", "Archivo", "Huella SHA-256", "Firma"),
               intro=f"<strong>Versión {version}, publicada el {FECHA['es']}.</strong> Antes de abrir un archivo, compara su huella con la de esta tabla y con la del <a href=\"https://github.com/guardianagroup/guardiana/blob/main/ledger.jsonl\">registro público</a>, donde se anotó antes que la descarga{{rekor}}.",
               sig="firma"),
    "en": dict(id="download", head=("System", "File", "SHA-256 hash", "Signature"),
               intro=f"<strong>Version {version}, published on {FECHA['en']}.</strong> Before you open a file, compare its hash with this table and with the <a href=\"https://github.com/guardianagroup/guardiana/blob/main/ledger.jsonl\">public ledger</a>, where it was written before the download existed{{rekor}}.",
               sig="signature"),
    "pt": dict(id="baixar", head=("Sistema", "Arquivo", "Impressão SHA-256", "Assinatura"),
               intro=f"<strong>Versão {version}, publicada em {FECHA['pt']}.</strong> Antes de abrir um arquivo, compare a impressão digital com a desta tabela e com a do <a href=\"https://github.com/guardianagroup/guardiana/blob/main/ledger.jsonl\">registro público</a>, onde foi anotada antes do download{{rekor}}.",
               sig="assinatura"),
}
REKOR = {
    "es": " (y en Rekor, entrada <code>{u}</code>)",
    "en": " (and in Rekor, entry <code>{u}</code>)",
    "pt": " (e no Rekor, entrada <code>{u}</code>)",
}
PAGES = {"es": "instalar.html", "en": "en/install.html", "pt": "pt/instalar.html"}

# The Mac app holds one native binary, built on the release Mac: say which chip it runs on
# instead of letting an Intel Mac download something that will not open.
CHIP = {
    "arm64": {"es": " · chip Apple (M1 o posterior)", "en": " · Apple chip (M1 or later)", "pt": " · chip Apple (M1 ou posterior)"},
    "x86_64": {"es": " · Intel", "en": " · Intel", "pt": " · Intel"},
}.get(os.environ.get("GUARDIANA_MAC_ARCH", "").strip())
if CHIP:
    for lang, rows in ROWS.items():
        ROWS[lang] = [(lab + CHIP[lang] if name.endswith("macos.app.zip") else lab, name) for lab, name in rows]

for lang, page in PAGES.items():
    t = TEXT[lang]
    rekor = REKOR[lang].format(u=html.escape(line["rekor_uuid"][:16])) if line.get("rekor_uuid") else ""
    rows = []
    for label, name in ROWS[lang]:
        if name not in files:
            continue
        sha = files[name]["sha256_signed"]
        rows.append(
            f'<tr><td>{label}</td><td><a href="/descargas/{version}/{name}" download>{name}</a><br><span class="muted">{size(name)}</span></td>'
            f'<td><code class="huella">{sha}</code></td><td><a href="/descargas/{version}/{name}.minisig">{t["sig"]}</a></td></tr>'
        )
    h = t["head"]
    block = (
        f'<div class="box" id="{t["id"]}"><p>{t["intro"].replace("{rekor}", rekor)}</p>\n'
        f'<div class="tabla-descargas"><table>\n<thead><tr><th>{h[0]}</th><th>{h[1]}</th><th>{h[2]}</th><th>{h[3]}</th></tr></thead>\n<tbody>\n'
        + "\n".join(rows)
        + "\n</tbody></table></div></div>"
    )
    p = site / page
    s = p.read_text(encoding="utf-8")
    new, n = re.subn(r'<div class="box"><p><strong>(Todavía no hay descarga|There is no download yet|Ainda não há download)\.</strong>.*?</p></div>', lambda m: block, s, count=1, flags=re.S)
    if n != 1:
        sys.exit(f"{page}: download box not found")
    if ".tabla-descargas" not in new:
        new = new.replace("</style>", ".tabla-descargas{overflow-x:auto}.tabla-descargas table{width:100%;border-collapse:collapse;font-size:.9rem}.tabla-descargas td,.tabla-descargas th{padding:.45rem .5rem;border-top:1px solid var(--linea,#ddd);text-align:left;vertical-align:top}code.huella{display:block;word-break:break-all;font-size:.75rem}@media (max-width:640px){.tabla-descargas thead{display:none}.tabla-descargas tr{display:block;border-top:1px solid var(--linea,#ddd);padding:.6rem 0}.tabla-descargas td{display:block;border:0;padding:.15rem 0}.tabla-descargas td:first-child{font-weight:600}}\n</style>", 1)
    p.write_text(new, encoding="utf-8")
    print(page, len(rows), "files")
PY
cd "$TRABAJO/web"
GUARDIANA_MAC_ARCH="$mac_arch" python3 "$TRABAJO/descargas.py" . "$TRABAJO/archivos" "$TRABAJO/archivos/ledger-line.json"
git add -A descargas instalar.html en/install.html pt/instalar.html
git commit -q -m "Downloads for $VERSION: files, hashes and signatures from ledger.jsonl"
if [ -n "$ENSAYO" ]; then
  echo "(ensayo: no se sube)"
  git show --stat HEAD | cat
else
  git push origin "HEAD:$RAMA_WEB" || para "no he podido subir la rama $RAMA_WEB de la web. Todo está en $TRABAJO. Díselo a Claude."
fi

echo
echo "HECHO. Firmado, anotado en el registro público y en Rekor, y la tabla de descargas preparada."
echo "Todo lo firmado está en $TRABAJO/archivos."
echo "Dile a Claude «ya está»: revisa la web y la publica."
