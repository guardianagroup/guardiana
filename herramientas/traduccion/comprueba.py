#!/usr/bin/env python3
"""Comprueba un trozo traducido contra su original: mismas claves, mismos marcadores {x}, sin vacíos, y lo que debe quedar
igual (GUARDIANA, ZERO, URLs). Uso: python3 comprueba.py <idioma> [NN ...] (sin NN: todos los que haya).
Sale con 1 si algo falla, y dice qué."""
import json, pathlib, re, sys

AQUI = pathlib.Path(__file__).resolve().parent
lang = sys.argv[1]
nums = sys.argv[2:] or sorted(p.name[:2] for p in (AQUI / 'trozos').glob('*.es.json'))
MARCA = re.compile(r'\{[A-Za-z_0-9]*\}')
ETIQ = re.compile(r'</?[a-z][a-z0-9]*\b')
URL = re.compile(r'https?://\S+')
mal = 0
for n in nums:
    es = json.loads((AQUI / 'trozos' / f'{n}.es.json').read_text(encoding='utf-8'))
    p = AQUI / lang / f'{n}.json'
    if not p.exists():
        print(f'{n}: falta'); mal += 1; continue
    try:
        tr = json.loads(p.read_text(encoding='utf-8'))
    except Exception as e:
        print(f'{n}: JSON roto: {e}'); mal += 1; continue
    faltan = [k for k in es if k not in tr]
    sobran = [k for k in tr if k not in es]
    if faltan: print(f'{n}: faltan {len(faltan)}: {faltan[:5]}'); mal += 1
    if sobran: print(f'{n}: sobran {len(sobran)}: {sobran[:5]}'); mal += 1
    for k, v in es.items():
        t = tr.get(k)
        if t is None: continue
        if not isinstance(t, str) or not t.strip():
            print(f'{n}: vacío {k}'); mal += 1; continue
        if sorted(MARCA.findall(v)) != sorted(MARCA.findall(t)):
            print(f'{n}: marcadores {k}: {MARCA.findall(v)} → {MARCA.findall(t)}'); mal += 1
        if sorted(ETIQ.findall(v)) != sorted(ETIQ.findall(t)):
            print(f'{n}: etiquetas {k}'); mal += 1
        for u in URL.findall(v):
            if u not in t: print(f'{n}: URL cambiada {k}: {u}'); mal += 1
        for marca in ('GUARDIANA', 'ZERO'):
            if marca in v and marca not in t:
                print(f'{n}: falta {marca} en {k}'); mal += 1
print('bien' if not mal else f'{mal} problemas')
sys.exit(1 if mal else 0)
