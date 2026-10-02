#!/usr/bin/env python3
"""What a careful customer does before opening a GUARDIANA download, done by a machine.

    python3 build/cliente/descarga.py <file name> [--version 1.0.1] [--dir DIR]

Downloads the file and its .minisig from the website, then checks the three promises the
install page makes, in the three languages:

  1. the SHA-256 of the file is the one written on the install page (es, en and pt),
  2. it is the one written in the public ledger (ledger.jsonl on GitHub), and the ledger
     names that version,
  3. the minisign signature verifies with the public key published on the page and in the
     repository (build/pubkey/minisign.pub), and its trusted comment names this file.

Every check prints a GitHub annotation (::notice / ::error) so the result can be read on the
run page and through the public API without signing in. Exit code 1 if any check failed.
The file is left in DIR for the install test that follows.

Written on 1 Oct 2026 for the customer test (.github/workflows/cliente.yml). No dependency but
`cryptography` (pip install cryptography) for Ed25519.
"""
import argparse
import base64
import hashlib
import json
import os
import re
import sys
import urllib.request

SITE = os.environ.get("GUARDIANA_SITE", "https://guardianagroup.com")
LEDGER = os.environ.get(
    "GUARDIANA_LEDGER",
    "https://raw.githubusercontent.com/guardianagroup/guardiana/main/ledger.jsonl",
)
PAGES = {"es": "/instalar.html", "en": "/en/install.html", "pt": "/pt/instalar.html"}

fallos = []


buenos = []


def ok(titulo, detalle=""):
    # A plain line in the log; all the good ones go out together at the end as one notice,
    # because GitHub keeps only ten notices per step and the failures must never be the ones lost.
    buenos.append(f"{titulo}: {detalle}" if detalle else titulo)
    print(f"  ok  {titulo} · {detalle}")


def mal(titulo, detalle=""):
    fallos.append(f"{titulo}: {detalle}" if detalle else titulo)
    print(f"::error title=FALLO · {titulo}::{detalle}")


def get(url, binary=False):
    req = urllib.request.Request(url, headers={"User-Agent": "guardiana-prueba-de-cliente"})
    with urllib.request.urlopen(req, timeout=60) as r:
        data = r.read()
    return data if binary else data.decode("utf-8", "replace")


def latest_version():
    lines = [l for l in get(LEDGER).splitlines() if l.strip().startswith("{")]
    return json.loads(lines[-1])["version"]


def ledger_entry(version):
    for line in get(LEDGER).splitlines():
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            d = json.loads(line)
        except ValueError:
            continue
        if d.get("version") == version:
            return d
    return None


# ----- minisign (Ed25519; "ED" = signature over the BLAKE2b-512 of the file) -----------------

def b64(line):
    return base64.b64decode(line.strip())


def parse_pubkey(text):
    lines = [l for l in text.strip().splitlines() if l.strip() and not l.startswith("untrusted")]
    raw = b64(lines[-1])
    if len(raw) != 42 or raw[:2] != b"Ed":
        raise ValueError("public key is not a minisign Ed25519 key")
    return raw[2:10], raw[10:]


def verify_minisign(pub_text, sig_text, data):
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey

    key_id, key = parse_pubkey(pub_text)
    lines = sig_text.splitlines()
    sig = b64(lines[1])
    algo, sig_key_id, signature = sig[:2], sig[2:10], sig[10:]
    if sig_key_id != key_id:
        raise ValueError("signed with another key (key id %s)" % sig_key_id[::-1].hex().upper())
    pk = Ed25519PublicKey.from_public_bytes(key)
    message = hashlib.blake2b(data).digest() if algo == b"ED" else data
    pk.verify(signature, message)
    trusted = lines[2]
    if not trusted.startswith("trusted comment: "):
        raise ValueError("no trusted comment")
    comment = trusted[len("trusted comment: "):]
    pk.verify(b64(lines[3]), signature + comment.encode())
    return comment, key_id[::-1].hex().upper()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("archivo")
    ap.add_argument("--version")
    ap.add_argument("--dir", default=".")
    a = ap.parse_args()
    version = a.version or latest_version()
    name = a.archivo.replace("{v}", version)
    os.makedirs(a.dir, exist_ok=True)
    print(f"Comprobando {name} (versión {version}) como lo haría un cliente")

    url = f"{SITE}/descargas/{version}/{name}"
    try:
        data = get(url, binary=True)
        open(os.path.join(a.dir, name), "wb").write(data)
        ok("descarga", f"{url} · {len(data)} bytes")
    except Exception as e:  # noqa: BLE001 - a customer just sees "it did not download"
        mal("descarga", f"{url}: {e}")
        return finish()
    digest = hashlib.sha256(data).hexdigest()

    # 1. The table of the install page, in the three languages. Only for the version the page
    #    offers today: an older one (kept for the record) is checked against the ledger and its
    #    signature, which is what still applies to it.
    actual = latest_version()
    for lang, path in (PAGES.items() if version == actual else []):
        try:
            html = get(SITE + path)
        except Exception as e:  # noqa: BLE001
            mal(f"página de instalar ({lang})", f"{path}: {e}")
            continue
        if name not in html:
            mal(f"página de instalar ({lang})", f"{path} no nombra {name}")
        elif digest not in html:
            near = re.findall(r"[0-9a-f]{64}", html[html.find(name):html.find(name) + 3000])
            mal(f"huella en la página ({lang})", f"{path}: el archivo da {digest}, la página dice {near[:1]}")
        else:
            ok(f"huella en la página ({lang})", f"{path} · {digest[:16]}…")
        if f"/descargas/{version}/{name}" not in html:
            mal(f"enlace de descarga ({lang})", f"{path} no enlaza /descargas/{version}/{name}")

    # 2. The public ledger.
    entry = ledger_entry(version)
    if not entry:
        mal("registro público", f"ledger.jsonl no tiene la versión {version}")
    else:
        f = next((x for x in entry.get("files", []) if x.get("name") == name), None)
        if not f:
            mal("registro público", f"la línea {version} no anota {name}")
        elif digest in (f.get("sha256_signed"), f.get("sha256_unsigned")):
            ok("registro público", f"versión {version}, commit {entry.get('commit', '')[:8]}, rekor {entry.get('rekor_uuid', '')[:16]}…")
        else:
            mal("registro público", f"el archivo da {digest}; el registro dice {f.get('sha256_signed') or f.get('sha256_unsigned')}")

    # 3. The signature, with the key of the page AND the key of the repository.
    try:
        sig_text = get(url + ".minisig")
    except Exception as e:  # noqa: BLE001
        mal("firma", f"no se pudo bajar {name}.minisig: {e}")
        return finish()
    repo_key = None
    here = os.path.dirname(os.path.abspath(__file__))
    try:
        repo_key = open(os.path.join(here, "..", "pubkey", "minisign.pub"), encoding="utf-8").read()
    except OSError:
        pass
    page_key = None
    m = re.search(r"RW[A-Za-z0-9+/]{54}", get(SITE + PAGES["es"]))
    if m:
        page_key = m.group(0)
    if repo_key and page_key and page_key not in repo_key:
        mal("clave pública", "la clave de la página no es la del repositorio")
    for origen, key in (("repositorio", repo_key), ("página", page_key)):
        if not key:
            mal(f"clave pública ({origen})", "no encontrada")
            continue
        try:
            comment, kid = verify_minisign(key, sig_text, data)
            if name not in comment or version not in comment:
                mal(f"firma ({origen})", f"verifica, pero el comentario de confianza dice «{comment}»")
            else:
                ok(f"firma ({origen})", f"clave {kid} · «{comment}»")
        except Exception as e:  # noqa: BLE001
            mal(f"firma ({origen})", str(e))
    return finish()


def finish():
    if buenos:
        print("::notice title=Descarga · lo que está bien (%d)::%s" % (len(buenos), "%0A".join(buenos)))
    print(f"\n{len(fallos)} fallos" + (": " + "; ".join(fallos) if fallos else ""))
    return 1 if fallos else 0


if __name__ == "__main__":
    sys.exit(main())
