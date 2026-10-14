#!/usr/bin/env python3
"""Where the servers and the person are, to the city when someone who knows says so (14 Oct 2026).

The owner: «saber la ciudad o el estado a dónde van… lo máximo que podáis de precisión», and «ser
muy precisos desde donde se conecta la persona». Two products, both bundled, never looked up
online by the program:

  nube-ip.bin   IP -> the place its operator publishes for it:
                · the cloud region of Amazon (ip-ranges.json), Google Cloud (cloud.json), Microsoft
                  Azure (Service Tags) and Oracle (public_ip_ranges.json), placed with the table of
                  regions in build/geo-regiones.tsv;
                · the city that DigitalOcean, Akamai/Linode and Vultr declare for each block in their
                  geofeeds (RFC 8805), placed with GeoNames.
  lugares.json  the places those indices point to: operator, region or city, country, lat, lon.
  ciudades.tsv  GeoNames «cities15000» (every city of 15 000 people or more), for the panel to name
                the city nearest to where the computer is (crates/panel/static/tierra/).
  geo.json      sources, dates, licences, SHA-256 of what was downloaded and of what was written,
                and the regions that came without a place (they are not placed: their points stay
                in their country).

Usage (CI has the internet; a release machine can do the same):
  python3 build/geo.py descargar --dir fuentes      downloads every source into fuentes/
  python3 build/geo.py convertir --dir fuentes      writes the products from fuentes/

Binary format (little-endian), read by crates/lists/src/lugar.rs, contiguous like ip-pais.bin:
  b"GNUBE001", u32 n4, n4 x u32 start, n4 x u16 index (0 = none, i + 1 = lugar i),
               u32 n6, n6 x u64 start (high 64 bits), n6 x u16 index
"""

import argparse
import csv
import hashlib
import importlib.util
import io
import ipaddress
import json
import math
import os
import re
import struct
import sys
import urllib.request
import zipfile

AQUI = os.path.dirname(os.path.abspath(__file__))
DATA = os.path.join(AQUI, '..', 'crates', 'lists', 'data')
TIERRA = os.path.join(AQUI, '..', 'crates', 'panel', 'static', 'tierra')
V4_MAX = (1 << 32) - 1
V6_MAX = (1 << 128) - 1

FUENTES = {
    'aws': ('https://ip-ranges.amazonaws.com/ip-ranges.json', 'aws.json'),
    'gcp': ('https://www.gstatic.com/ipranges/cloud.json', 'gcp.json'),
    'oracle': ('https://docs.oracle.com/en-us/iaas/tools/public_ip_ranges.json', 'oracle.json'),
    'do': ('https://digitalocean.com/geo/google.csv', 'do.csv'),
    'linode': ('https://geoip.linode.com/', 'linode.csv'),
    'vultr': ('https://geofeed.constant.com/?text', 'vultr.csv'),
    'ciudades': ('https://download.geonames.org/export/dump/cities15000.zip', 'cities15000.zip'),
    'admin1': ('https://download.geonames.org/export/dump/admin1CodesASCII.txt', 'admin1CodesASCII.txt'),
}
# Azure publishes its Service Tags under a link that changes every week; the stable page names it.
AZURE_PAGINA = 'https://www.microsoft.com/en-us/download/details.aspx?id=56519'
LICENCIAS = {
    'aws': 'publicado por Amazon Web Services para que sus clientes configuren cortafuegos',
    'gcp': 'publicado por Google Cloud para que sus clientes configuren cortafuegos',
    'azure': 'publicado por Microsoft (Azure IP Ranges and Service Tags – Public Cloud)',
    'oracle': 'publicado por Oracle Cloud Infrastructure',
    'do': 'geofeed RFC 8805 publicado por DigitalOcean',
    'linode': 'geofeed RFC 8805 publicado por Akamai (Linode)',
    'vultr': 'geofeed RFC 8805 publicado por Vultr (Constant)',
    'ciudades': 'GeoNames, CC BY 4.0 (https://www.geonames.org)',
    'admin1': 'GeoNames, CC BY 4.0 (https://www.geonames.org)',
}
UA = {'User-Agent': 'GUARDIANA-build/1.0 (+https://guardianagroup.com)'}


def sha256(b):
    return hashlib.sha256(b).hexdigest()


def baja(url):
    with urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=120) as r:
        return r.read()


def descargar(d):
    os.makedirs(d, exist_ok=True)
    hechos = {}
    for k, (url, nombre) in FUENTES.items():
        try:
            b = baja(url)
            open(os.path.join(d, nombre), 'wb').write(b)
            hechos[k] = {'url': url, 'archivo': nombre, 'sha256': sha256(b), 'bytes': len(b)}
            print(f'{k}: {len(b)} bytes')
        except Exception as e:  # a source that fails is said, and the others go on
            print(f'::warning title=geo {k}::{url}: {e}')
    try:
        pagina = baja(AZURE_PAGINA).decode('utf-8', 'replace')
        m = re.search(r'https://download\.microsoft\.com/download/[^"\']+ServiceTags_Public_\d+\.json', pagina)
        if not m:
            raise RuntimeError('no se encontró el enlace de ServiceTags_Public en la página')
        b = baja(m.group(0))
        open(os.path.join(d, 'azure.json'), 'wb').write(b)
        hechos['azure'] = {'url': m.group(0), 'pagina': AZURE_PAGINA, 'archivo': 'azure.json', 'sha256': sha256(b), 'bytes': len(b)}
        print(f'azure: {len(b)} bytes')
    except Exception as e:
        print(f'::warning title=geo azure::{e}')
    json.dump(hechos, open(os.path.join(d, 'descargas.json'), 'w'), indent=2)


def red_ip():
    """build/red-ip.py's range helpers, so both tables are made the same way."""
    spec = importlib.util.spec_from_file_location('red_ip', os.path.join(AQUI, 'red-ip.py'))
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


def ciudades(d):
    """GeoNames cities15000 with their first-level division (state, department) by name."""
    a1 = {}
    p = os.path.join(d, 'admin1CodesASCII.txt')
    if os.path.exists(p):
        for linea in open(p, encoding='utf-8'):
            c = linea.rstrip('\n').split('\t')
            if len(c) >= 2:
                a1[c[0]] = c[1]
    out = []
    with zipfile.ZipFile(os.path.join(d, 'cities15000.zip')) as z:
        for linea in z.read('cities15000.txt').decode('utf-8').splitlines():
            c = linea.split('\t')
            # geonameid, name, asciiname, alternatenames, lat, lon, fclass, fcode, cc, cc2, admin1, ..., population
            nombre, lat, lon, cc, adm, pob = c[1], float(c[4]), float(c[5]), c[8], c[10], int(c[14] or 0)
            out.append((nombre, cc, a1.get(f'{cc}.{adm}', ''), round(lat, 2), round(lon, 2), pob, c[2]))
    return out


def norm(s):
    import unicodedata
    s = unicodedata.normalize('NFKD', s).encode('ascii', 'ignore').decode().lower()
    return re.sub(r'[^a-z0-9]+', ' ', s).strip()


def convertir(d):
    R = red_ip()
    lugares, indice = [], {}
    sin_sitio = {}

    def lugar(proveedor, region, ciudad, cc, lat, lon, fuente):
        k = (proveedor, region)
        if k not in indice:
            indice[k] = len(lugares) + 1
            lugares.append({'proveedor': proveedor, 'region': region, 'ciudad': ciudad, 'pais': cc, 'lat': lat, 'lon': lon, 'fuente': fuente})
        return indice[k]

    regiones = {}
    for linea in open(os.path.join(AQUI, 'geo-regiones.tsv'), encoding='utf-8'):
        if linea.startswith('#') or not linea.strip():
            continue
        p, r, ciudad, cc, lat, lon = linea.rstrip('\n').split('\t')
        regiones[(p, r)] = (ciudad, cc, float(lat), float(lon))

    v4, v6 = [], []

    def anota(red, idx):
        n = ipaddress.ip_network(red, strict=False)
        (v4 if n.version == 4 else v6).append((int(n.network_address), int(n.broadcast_address), idx))

    def region(p, r, prefijo):
        if not r or r.lower() in ('global', ''):
            return
        if (p, r) not in regiones:
            sin_sitio.setdefault(f'{p}:{r}', 0)
            sin_sitio[f'{p}:{r}'] += 1
            return
        ciudad, cc, lat, lon = regiones[(p, r)]
        anota(prefijo, lugar(p, r, ciudad, cc, lat, lon, 'región del operador'))

    cuentas = {}
    ruta = lambda n: os.path.join(d, n)
    if os.path.exists(ruta('aws.json')):
        j = json.load(open(ruta('aws.json')))
        for x in j.get('prefixes', []):
            region('aws', x.get('region'), x['ip_prefix'])
        for x in j.get('ipv6_prefixes', []):
            region('aws', x.get('region'), x['ipv6_prefix'])
        cuentas['aws'] = len(j.get('prefixes', [])) + len(j.get('ipv6_prefixes', []))
    if os.path.exists(ruta('gcp.json')):
        j = json.load(open(ruta('gcp.json')))
        for x in j.get('prefixes', []):
            region('gcp', x.get('scope'), x.get('ipv4Prefix') or x.get('ipv6Prefix'))
        cuentas['gcp'] = len(j.get('prefixes', []))
    if os.path.exists(ruta('azure.json')):
        j = json.load(open(ruta('azure.json')))
        n = 0
        for v in j.get('values', []):
            pr = v.get('properties', {})
            # Only the tags of one region (AzureCloud.eastus…): the global ones say no place.
            if not pr.get('region') or not v.get('name', '').startswith('AzureCloud.'):
                continue
            for pref in pr.get('addressPrefixes', []):
                region('azure', pr['region'], pref)
                n += 1
        cuentas['azure'] = n
    if os.path.exists(ruta('oracle.json')):
        j = json.load(open(ruta('oracle.json')))
        n = 0
        for reg in j.get('regions', []):
            for c in reg.get('cidrs', []):
                region('oracle', reg.get('region'), c['cidr'])
                n += 1
        cuentas['oracle'] = n

    # Geofeeds: the operator declares the city of each block; GeoNames gives its coordinates.
    tabla = ciudades(d)
    porNombre = {}
    for (nombre, cc, adm, lat, lon, pob, ascii_) in tabla:
        for k in {norm(nombre), norm(ascii_)}:
            porNombre.setdefault((k, cc), []).append((pob, nombre, adm, lat, lon))
    for p, archivo in (('do', 'do.csv'), ('linode', 'linode.csv'), ('vultr', 'vultr.csv')):
        if not os.path.exists(ruta(archivo)):
            continue
        n = sin = 0
        for fila in csv.reader(io.StringIO(open(ruta(archivo), encoding='utf-8', errors='replace').read())):
            if not fila or fila[0].startswith('#') or len(fila) < 4:
                continue
            pref, cc, _reg, ciudad = fila[0].strip(), fila[1].strip().upper(), fila[2].strip(), fila[3].strip()
            cand = porNombre.get((norm(ciudad), cc))
            if not ciudad or not cand:
                sin += 1
                continue
            pob, nombre, adm, lat, lon = max(cand)
            try:
                anota(pref, lugar(p, f'{nombre}|{cc}', nombre + (f', {adm}' if adm else ''), cc, lat, lon, 'geofeed del operador'))
                n += 1
            except ValueError:
                sin += 1
        cuentas[p] = n
        if sin:
            sin_sitio[f'{p}:ciudades sin encontrar en GeoNames'] = sin

    assert len(lugares) < 65535, len(lugares)

    def tabla_bin(rangos, maximo):
        rangos.sort()
        limpios, fin = [], -1
        for a, b, v in rangos:  # overlaps: the first (most specific comes later, keep simple: skip)
            if a <= fin:
                a = fin + 1
                if a > b:
                    continue
            limpios.append((a, b, v))
            fin = b
        return R.contiguos(limpios, maximo)

    t4 = tabla_bin(v4, V4_MAX)
    t6 = tabla_bin([(a >> 64, b >> 64, v) for a, b, v in v6], (1 << 64) - 1)
    out = bytearray(b'GNUBE001')
    out += struct.pack('<I', len(t4)) + b''.join(struct.pack('<I', a) for a, _ in t4) + b''.join(struct.pack('<H', v) for _, v in t4)
    out += struct.pack('<I', len(t6)) + b''.join(struct.pack('<Q', a) for a, _ in t6) + b''.join(struct.pack('<H', v) for _, v in t6)
    open(os.path.join(DATA, 'nube-ip.bin'), 'wb').write(out)
    json.dump(lugares, open(os.path.join(DATA, 'lugares.json'), 'w', encoding='utf-8'), ensure_ascii=False, indent=0)

    # The panel's city list: name, country, division, lat, lon; the biggest first, so the nearest
    # search can stop early on ties.
    os.makedirs(TIERRA, exist_ok=True)
    with open(os.path.join(TIERRA, 'ciudades.tsv'), 'w', encoding='utf-8', newline='\n') as f:
        f.write('# GeoNames cities15000, CC BY 4.0, https://www.geonames.org — nombre, país, división, lat, lon\n')
        for (nombre, cc, adm, lat, lon, pob, _a) in sorted(tabla, key=lambda x: -x[5]):
            f.write(f'{nombre}\t{cc}\t{adm}\t{lat}\t{lon}\n')

    desc = json.load(open(ruta('descargas.json'))) if os.path.exists(ruta('descargas.json')) else {}
    meta = {
        'hecho': __import__('datetime').datetime.now(__import__('datetime').timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ'),
        'fuentes': {k: dict(v, licencia=LICENCIAS.get(k, '')) for k, v in desc.items()},
        'prefijos': cuentas,
        'lugares': len(lugares),
        'rangos_v4': len(t4), 'rangos_v6': len(t6),
        'ciudades': len(tabla),
        'sin_sitio': dict(sorted(sin_sitio.items(), key=lambda x: -x[1])),
        'sha256': {
            'nube-ip.bin': sha256(bytes(out)),
            'lugares.json': sha256(open(os.path.join(DATA, 'lugares.json'), 'rb').read()),
            'ciudades.tsv': sha256(open(os.path.join(TIERRA, 'ciudades.tsv'), 'rb').read()),
        },
    }
    json.dump(meta, open(os.path.join(DATA, 'geo.json'), 'w', encoding='utf-8'), ensure_ascii=False, indent=2)
    print(json.dumps({k: meta[k] for k in ('prefijos', 'lugares', 'rangos_v4', 'rangos_v6', 'ciudades')}, ensure_ascii=False))
    for k, v in list(meta['sin_sitio'].items())[:60]:
        print(f'::notice title=sin sitio::{k} ({v})')


if __name__ == '__main__':
    ap = argparse.ArgumentParser()
    ap.add_argument('que', choices=['descargar', 'convertir'])
    ap.add_argument('--dir', default='fuentes-geo')
    a = ap.parse_args()
    (descargar if a.que == 'descargar' else convertir)(a.dir)
