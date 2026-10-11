#!/usr/bin/env python3
"""Refresh the IP -> country and IP -> network (ASN) data bundled with GUARDIANA (decision 203).

Run by a person before a release, like build/fetch-lists.sh. The program itself never downloads
anything for this: it looks addresses up in the copy embedded at build time, so nobody outside
the house learns which servers the house talked to.

Source: DB-IP "IP to Country Lite" and "IP to ASN Lite", monthly, CC BY 4.0
(https://db-ip.com/db/lite.php). Attribution required: "IP Geolocation by DB-IP", with a link to
https://db-ip.com on the pages that show the results.

Usage:
  python3 build/red-ip.py descargar 2026-10
      downloads both monthly files from download.db-ip.com and converts them.
  python3 build/red-ip.py convertir PAIS.csv[.gz] RED.csv[.gz] --fecha 2026-10 [--muestra]
      converts files already on disk. --muestra marks the result as a test sample: the panel
      then says, on every page that shows it, that these are not real DB-IP data.

Output, in crates/lists/data/:
  ip-pais.bin   IP -> two-letter country, sorted ranges (format below)
  ip-red.bin    IP -> autonomous system number and its organisation name
  red-ip.json   source, month, licence, SHA-256 of the downloaded files and of the outputs

Binary format (little-endian), read by crates/lists/src/red.rs. Ranges are contiguous: each
entry is the first address of a range that ends where the next one starts, and value 0 means
"no data" (gaps in the source become such ranges). Adjacent ranges with the same value are
merged. IPv6 is kept at /64 granularity (the high 64 bits): a source boundary inside a /64 is
moved to the next /64, and a range that does not reach one is dropped; how many is written in
red-ip.json (`v6_por_debajo_de_64`).

  ip-pais.bin: b"GIPPAIS1", u32 ncodes, ncodes x 2 ASCII bytes (index 0 is "--", no data),
               u32 n4, n4 x u32 start, n4 x u8 index, u32 n6, n6 x u64 start, n6 x u8 index
  ip-red.bin:  b"GIPRED01", u32 nasn, nasn x u32 asn (ascending), (nasn + 1) x u32 offset into
               the names, u32 names length, names (UTF-8), u32 n4, n4 x u32 start,
               n4 x 3-byte index (0 = none, i + 1 = asn i), u32 n6, n6 x u64 start, n6 x 3-byte index
"""

import argparse
import collections
import csv
import gzip
import hashlib
import io
import ipaddress
import json
import os
import struct
import sys
import urllib.request

DATA = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'crates', 'lists', 'data')
URL_PAIS = 'https://download.db-ip.com/free/dbip-country-lite-{fecha}.csv.gz'
URL_RED = 'https://download.db-ip.com/free/dbip-asn-lite-{fecha}.csv.gz'
V4_MAX = (1 << 32) - 1
V6_MAX = (1 << 128) - 1


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def leer(path):
    """The bytes of a file as given, and its text (gunzipped when it ends in .gz)."""
    with open(path, 'rb') as f:
        raw = f.read()
    text = gzip.decompress(raw) if path.endswith('.gz') else raw
    return raw, text.decode('utf-8')


def filas(text):
    for row in csv.reader(io.StringIO(text)):
        if row and not row[0].startswith('#'):
            yield row


def contiguos(rangos, maximo):
    """Sorted, non-overlapping (start, end, value) -> contiguous (start, value) covering 0..maximo,
    with 0 for the gaps and equal neighbours merged."""
    out = []
    cur = 0
    for start, end, val in sorted(rangos):
        if end < cur:
            continue  # wholly inside what is already covered: the source overlaps itself
        start = max(start, cur)
        if start > cur:
            out.append((cur, 0))
        out.append((start, val))
        cur = end + 1
        if cur > maximo:
            break
    if cur <= maximo:
        out.append((cur, 0))
    return fusionar(out)


def fusionar(segmentos):
    out = []
    for start, val in segmentos:
        if out and out[-1][1] == val:
            continue
        out.append((start, val))
    return out


def a_64(segmentos):
    """IPv6 segments (start, value) -> /64 segments; returns them and how many did not fit."""
    out = []
    perdidos = 0
    for i, (start, val) in enumerate(segmentos):
        nxt = segmentos[i + 1][0] if i + 1 < len(segmentos) else V6_MAX + 1
        alto = start >> 64
        if start & ((1 << 64) - 1):
            alto += 1
        if (alto << 64) >= nxt:
            perdidos += 1
            continue
        if out and out[-1][0] == alto:
            out[-1] = (alto, val)
        else:
            out.append((alto, val))
    return fusionar(out), perdidos


def separar(text, valor):
    """CSV rows -> ([v4 ranges], [v6 ranges]) with valor(row) as value; rows it rejects are
    skipped."""
    v4, v6 = [], []
    for row in filas(text):
        a = ipaddress.ip_address(row[0].strip())
        b = ipaddress.ip_address(row[1].strip())
        if a.version != b.version or int(b) < int(a):
            raise SystemExit(f'rango imposible: {row[:2]}')
        v = valor(row)
        if v is None:
            continue
        (v4 if a.version == 4 else v6).append((int(a), int(b), v))
    return v4, v6


def u24(n):
    return struct.pack('<I', n)[:3]


def convertir_pais(text):
    v4, v6 = separar(text, lambda r: r[2].strip().upper() if len(r) > 2 and len(r[2].strip()) == 2 and r[2].strip().upper() != 'ZZ' else None)
    codigos = ['--'] + sorted({v for _, _, v in v4 + v6})
    idx = {c: i for i, c in enumerate(codigos)}
    if len(codigos) > 255:
        raise SystemExit('más de 254 países: no cabe en un byte')
    s4 = contiguos([(a, b, idx[v]) for a, b, v in v4], V4_MAX)
    s6, perdidos = a_64(contiguos([(a, b, idx[v]) for a, b, v in v6], V6_MAX))
    out = bytearray(b'GIPPAIS1')
    out += struct.pack('<I', len(codigos))
    for c in codigos:
        out += c.encode('ascii')
    out += struct.pack('<I', len(s4))
    out += b''.join(struct.pack('<I', s) for s, _ in s4)
    out += bytes(v for _, v in s4)
    out += struct.pack('<I', len(s6))
    out += b''.join(struct.pack('<Q', s) for s, _ in s6)
    out += bytes(v for _, v in s6)
    info = {'registros_origen': len(v4) + len(v6), 'rangos_v4': len(s4), 'rangos_v6': len(s6),
            'paises': len(codigos) - 1, 'v6_por_debajo_de_64': perdidos}
    return bytes(out), info


def convertir_red(text):
    def valor(r):
        if len(r) < 3 or not r[2].strip().isdigit():
            return None
        return int(r[2].strip())
    nombres = collections.defaultdict(collections.Counter)
    for r in filas(text):
        if len(r) >= 4 and r[2].strip().isdigit():
            nombres[int(r[2].strip())][r[3].strip()] += 1
    v4, v6 = separar(text, valor)
    asns = sorted({v for _, _, v in v4 + v6})
    idx = {a: i + 1 for i, a in enumerate(asns)}
    if len(asns) >= (1 << 24):
        raise SystemExit('demasiadas redes para 3 bytes')
    s4 = contiguos([(a, b, idx[v]) for a, b, v in v4], V4_MAX)
    s6, perdidos = a_64(contiguos([(a, b, idx[v]) for a, b, v in v6], V6_MAX))
    blob = bytearray()
    offsets = []
    for a in asns:
        offsets.append(len(blob))
        # The organisation name the source gives most often for this number.
        nombre = max(sorted(nombres[a].items()), key=lambda kv: kv[1])[0] if nombres[a] else ''
        blob += nombre.encode('utf-8')
    offsets.append(len(blob))
    out = bytearray(b'GIPRED01')
    out += struct.pack('<I', len(asns))
    out += b''.join(struct.pack('<I', a) for a in asns)
    out += b''.join(struct.pack('<I', o) for o in offsets)
    out += struct.pack('<I', len(blob)) + blob
    out += struct.pack('<I', len(s4))
    out += b''.join(struct.pack('<I', s) for s, _ in s4)
    out += b''.join(u24(v) for _, v in s4)
    out += struct.pack('<I', len(s6))
    out += b''.join(struct.pack('<Q', s) for s, _ in s6)
    out += b''.join(u24(v) for _, v in s6)
    info = {'registros_origen': len(v4) + len(v6), 'rangos_v4': len(s4), 'rangos_v6': len(s6),
            'redes': len(asns), 'v6_por_debajo_de_64': perdidos}
    return bytes(out), info


def escribir(pais_path, red_path, fecha, muestra, url_pais, url_red, salida):
    raw_p, text_p = leer(pais_path)
    raw_r, text_r = leer(red_path)
    bin_p, info_p = convertir_pais(text_p)
    bin_r, info_r = convertir_red(text_r)
    os.makedirs(salida, exist_ok=True)
    with open(os.path.join(salida, 'ip-pais.bin'), 'wb') as f:
        f.write(bin_p)
    with open(os.path.join(salida, 'ip-red.bin'), 'wb') as f:
        f.write(bin_r)
    manifest = {
        'fuente': 'DB-IP Lite',
        'fecha': fecha,
        'licencia': 'CC BY 4.0',
        'licencia_url': 'https://creativecommons.org/licenses/by/4.0/',
        'atribucion': 'IP Geolocation by DB-IP',
        'web': 'https://db-ip.com',
        'muestra': muestra,
        'pais': dict(url=url_pais, archivo_origen=os.path.basename(pais_path),
                     sha256_origen=sha256(raw_p), archivo='ip-pais.bin', sha256=sha256(bin_p), **info_p),
        'red': dict(url=url_red, archivo_origen=os.path.basename(red_path),
                    sha256_origen=sha256(raw_r), archivo='ip-red.bin', sha256=sha256(bin_r), **info_r),
    }
    with open(os.path.join(salida, 'red-ip.json'), 'w', encoding='utf-8') as f:
        json.dump(manifest, f, ensure_ascii=False, indent=2)
        f.write('\n')
    print(json.dumps(manifest, ensure_ascii=False, indent=2))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest='orden', required=True)
    d = sub.add_parser('descargar')
    d.add_argument('fecha', help='AAAA-MM, el mes de la base')
    d.add_argument('--carpeta', default=os.path.join(DATA, '..', '..', '..', 'target', 'red-ip'))
    c = sub.add_parser('convertir')
    c.add_argument('pais')
    c.add_argument('red')
    c.add_argument('--fecha', required=True)
    c.add_argument('--muestra', action='store_true')
    c.add_argument('--salida', default=DATA)
    a = ap.parse_args()
    if a.orden == 'descargar':
        os.makedirs(a.carpeta, exist_ok=True)
        rutas = []
        for plantilla in (URL_PAIS, URL_RED):
            url = plantilla.format(fecha=a.fecha)
            ruta = os.path.join(a.carpeta, url.rsplit('/', 1)[1])
            print('bajando', url, file=sys.stderr)
            with urllib.request.urlopen(url) as r, open(ruta, 'wb') as f:
                f.write(r.read())
            rutas.append((ruta, url))
        escribir(rutas[0][0], rutas[1][0], a.fecha, False, rutas[0][1], rutas[1][1], DATA)
    else:
        escribir(a.pais, a.red, a.fecha, a.muestra, URL_PAIS.format(fecha=a.fecha),
                 URL_RED.format(fecha=a.fecha), a.salida)


if __name__ == '__main__':
    main()
