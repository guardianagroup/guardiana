#!/usr/bin/env python3
"""The Earth up close, for «el globo en directo» (15 Oct 2026).

The owner: «en el mapa… pierde resolución cuando hago zoom». The globe carries one picture of the
whole Earth at 4096 × 2048 (crates/panel/static/tierra/dia.jpg, luces.jpg): 11 pixels per degree,
which is fine for the whole planet and blurry over a country. This script cuts the NASA pictures
at their full size into tiles of 45° × 45° (8 columns × 4 rows), 2048 × 2048 each: 45 pixels per
degree, four times sharper. The page loads only the tiles under the view, and only when the view
is close (por-dentro.js, «detalle»).

Sources (public domain, NASA):
  day     Blue Marble Next Generation (July), with topography and bathymetry, 21 600 × 10 800
          (NASA Earth Observatory, Reto Stöckli et al.)
  night   Black Marble 2016, 13 500 × 6 750 (NASA Earth Observatory / Suomi NPP VIIRS)
Each tile keeps a bleed of 8 pixels from its neighbours on every side, so the seams between tiles
cannot show; the page maps 45° onto the inner 2032 pixels.

Usage (CI has the internet):
  python3 build/tierra.py descargar --dir fuentes-tierra
  python3 build/tierra.py cortar --dir fuentes-tierra        writes crates/panel/static/tierra/t/
"""

import argparse
import hashlib
import json
import os
import sys
import urllib.request

from PIL import Image

Image.MAX_IMAGE_PIXELS = None

RAIZ = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DESTINO = os.path.join(RAIZ, 'crates', 'panel', 'static', 'tierra', 't')
COLS, FILAS, LADO, SANGRE = 8, 4, 2048, 8
UTIL = LADO - 2 * SANGRE  # 2032: the pixels of a tile that are its own 45°

# Several addresses for each picture: NASA moves them from time to time. The first that answers
# is used and recorded in fuentes.json.
FUENTES = {
    'dia': [
        'https://eoimages.gsfc.nasa.gov/images/imagerecords/73000/73751/world.topo.bathy.200407.3x21600x10800.jpg',
        'https://eoimages.gsfc.nasa.gov/images/imagerecords/73000/73751/world.topo.bathy.200407.3x21600x10800.png',
        'https://eoimages.gsfc.nasa.gov/images/imagerecords/73000/73776/world.topo.bathy.200408.3x21600x10800.jpg',
        'https://eoimages.gsfc.nasa.gov/images/imagerecords/73000/73751/world.topo.bathy.200407.3x5400x2700.jpg',
    ],
    'noche': [
        'https://eoimages.gsfc.nasa.gov/images/imagerecords/144000/144898/BlackMarble_2016_3km.jpg',
        'https://eoimages.gsfc.nasa.gov/images/imagerecords/144000/144898/BlackMarble_2016_3km_gray.jpg',
        'https://eoimages.gsfc.nasa.gov/images/imagerecords/144000/144898/BlackMarble_2016_01deg.jpg',
    ],
}


def sha256(ruta):
    h = hashlib.sha256()
    with open(ruta, 'rb') as f:
        for trozo in iter(lambda: f.read(1 << 20), b''):
            h.update(trozo)
    return h.hexdigest()


def descargar(d):
    os.makedirs(d, exist_ok=True)
    registro = {}
    for clave, urls in FUENTES.items():
        for url in urls:
            ruta = os.path.join(d, clave + os.path.splitext(url)[1])
            try:
                print('descargo', url, flush=True)
                req = urllib.request.Request(url, headers={'User-Agent': 'guardiana-tierra/1 (+https://guardianagroup.com)'})
                with urllib.request.urlopen(req, timeout=120) as r, open(ruta, 'wb') as f:
                    while True:
                        trozo = r.read(1 << 20)
                        if not trozo:
                            break
                        f.write(trozo)
                with Image.open(ruta) as im:
                    tam = im.size
                registro[clave] = {'url': url, 'archivo': os.path.basename(ruta), 'tamano': tam, 'sha256': sha256(ruta), 'bytes': os.path.getsize(ruta)}
                print('  ok', tam, os.path.getsize(ruta), 'bytes', flush=True)
                break
            except Exception as e:  # noqa: BLE001 - try the next address
                print('  no:', e, flush=True)
                if os.path.exists(ruta):
                    os.remove(ruta)
        if clave not in registro:
            print('sin fuente para', clave, file=sys.stderr)
            sys.exit(1)
    with open(os.path.join(d, 'descargas.json'), 'w', encoding='utf-8') as f:
        json.dump(registro, f, indent=1, ensure_ascii=False)


def luces(im):
    """Only the lights, in grey: the night picture without its dark blue seas and black land."""
    g = im.convert('L')
    # Black Marble's background sits around 10-20 of 255; everything below is not a light.
    return g.point(lambda v: 0 if v < 24 else min(255, int((v - 24) * 255 / (255 - 24))))


def cortar(d):
    with open(os.path.join(d, 'descargas.json'), encoding='utf-8') as f:
        registro = json.load(f)
    os.makedirs(DESTINO, exist_ok=True)
    salida = {'cols': COLS, 'filas': FILAS, 'lado': LADO, 'sangre': SANGRE, 'fuentes': registro, 'tiles': {}}
    for clave, modo, calidad in (('dia', 'RGB', 84), ('noche', 'L', 82)):
        im = Image.open(os.path.join(d, registro[clave]['archivo']))
        im = im.convert('RGB') if modo == 'RGB' else luces(im)
        W, H = im.size
        # Work at the size the tiles need: COLS*UTIL × FILAS*UTIL. Downscale with Lanczos if the
        # source is larger, upscale (bicubic) only when it is smaller.
        ancho, alto = COLS * UTIL, FILAS * UTIL
        if (W, H) != (ancho, alto):
            im = im.resize((ancho, alto), Image.LANCZOS if W > ancho else Image.BICUBIC)
        W, H = im.size
        # A strip wider than the Earth, so the bleed wraps around the date line.
        ancha = Image.new(modo, (W + 2 * SANGRE, H))
        ancha.paste(im, (SANGRE, 0))
        ancha.paste(im.crop((W - SANGRE, 0, W, H)), (0, 0))
        ancha.paste(im.crop((0, 0, SANGRE, H)), (W + SANGRE, 0))
        nombre = 'dia' if clave == 'dia' else 'luces'
        for r in range(FILAS):
            for c in range(COLS):
                x0, y0 = c * UTIL, r * UTIL - SANGRE
                caja = Image.new(modo, (LADO, LADO))
                ya, yb = max(0, y0), min(H, y0 + LADO)
                caja.paste(ancha.crop((x0, ya, x0 + LADO, yb)), (0, ya - y0))
                # Beyond the poles, repeat the edge row.
                if y0 < 0:
                    caja.paste(ancha.crop((x0, 0, x0 + LADO, 1)).resize((LADO, -y0)), (0, 0))
                if y0 + LADO > H:
                    caja.paste(ancha.crop((x0, H - 1, x0 + LADO, H)).resize((LADO, y0 + LADO - H)), (0, H - y0))
                ruta = os.path.join(DESTINO, f'{nombre}-{c}-{r}.jpg')
                caja.save(ruta, 'JPEG', quality=calidad, optimize=True, progressive=False, subsampling=0 if modo == 'L' else 2)
                salida['tiles'][f'{nombre}-{c}-{r}'] = {'bytes': os.path.getsize(ruta), 'sha256': sha256(ruta)}
                print(ruta, os.path.getsize(ruta), flush=True)
    with open(os.path.join(DESTINO, 'tiles.json'), 'w', encoding='utf-8') as f:
        json.dump(salida, f, indent=1, ensure_ascii=False)
    total = sum(t['bytes'] for t in salida['tiles'].values())
    print('total', total, 'bytes en', len(salida['tiles']), 'tiles')


def main():
    p = argparse.ArgumentParser()
    p.add_argument('orden', choices=['descargar', 'cortar'])
    p.add_argument('--dir', default='fuentes-tierra')
    a = p.parse_args()
    (descargar if a.orden == 'descargar' else cortar)(a.dir)


if __name__ == '__main__':
    main()
