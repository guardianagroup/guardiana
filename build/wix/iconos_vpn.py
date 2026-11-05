#!/usr/bin/env python3
"""The VPN's icons next to Windows' clock (5 Nov 2026; the owner, 3 Nov: «Hazlo claro»): GUARDIANA's own icon
(guardiana.ico, every size it has) with a dot at the bottom right in the VPN's colour: green connected, grey off, amber
connecting, red when its server does not answer. A thin ring of the brand's near-black around the dot keeps it apart from
the padlock and from a light taskbar. Saved next to this file as vpn-<colour>.ico; GUARDIANA ZERO carries them
(zero/navegador/src/avisos.rs).
Usage: python3 build/wix/iconos_vpn.py"""
import pathlib
from PIL import Image, ImageDraw
D = pathlib.Path(__file__).resolve().parent
COLORES = {'verde': (52, 199, 123), 'gris': (142, 153, 166), 'ambar': (242, 179, 61), 'rojo': (229, 72, 77)}
TAMANOS = [16, 20, 24, 32, 40, 48, 64]
base = Image.open(D / 'guardiana.ico')
def con_punto(lado, color):
    base.size = (lado, lado)
    im = base.convert('RGBA').copy()
    # drawn four times bigger and reduced, so the dot's edge is smooth at 16 px too
    k = 4
    capa = Image.new('RGBA', (lado * k, lado * k), (0, 0, 0, 0))
    d = ImageDraw.Draw(capa)
    r = lado * k * 0.25
    cx = cy = lado * k - r - lado * k * 0.02
    anillo = max(k, lado * k * 0.06)
    d.ellipse((cx - r - anillo, cy - r - anillo, cx + r + anillo, cy + r + anillo), fill=(6, 8, 12, 255))
    d.ellipse((cx - r, cy - r, cx + r, cy + r), fill=color + (255,))
    capa = capa.resize((lado, lado), Image.LANCZOS)
    im.alpha_composite(capa)
    return im
for nombre, color in COLORES.items():
    imagenes = [con_punto(t, color) for t in TAMANOS]
    imagenes[-1].save(D / f'vpn-{nombre}.ico', sizes=[(t, t) for t in TAMANOS], append_images=imagenes[:-1])
print('ok')
