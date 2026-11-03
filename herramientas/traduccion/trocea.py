#!/usr/bin/env python3
"""Parte los textos del programa en trozos para traducir (2 nov 2026, 1.0.54: el programa en todos los idiomas).

Fuentes: crates/panel/i18n/es.json (secciones con textos) y crates/zero/i18n/es.json (plano), del repositorio del programa.
Cada trozo es un JSON plano {"p|seccion|clave": "texto en español", ...} o {"z|clave": ...}, con unas 2.500 palabras, y
al lado el inglés de las mismas claves (referencia). Salida: trozos/NN.es.json y trozos/NN.en.json.

    python3 trocea.py /home/claude/guardiana-av
"""
import json, pathlib, sys

PROG = pathlib.Path(sys.argv[1])
AQUI = pathlib.Path(__file__).resolve().parent
SAL = AQUI / 'trozos'
SAL.mkdir(exist_ok=True)

def plano(raiz):
    es_p = json.loads((raiz / 'crates/panel/i18n/es.json').read_text(encoding='utf-8'))
    en_p = json.loads((raiz / 'crates/panel/i18n/en.json').read_text(encoding='utf-8'))
    es_z = json.loads((raiz / 'crates/zero/i18n/es.json').read_text(encoding='utf-8'))
    en_z = json.loads((raiz / 'crates/zero/i18n/en.json').read_text(encoding='utf-8'))
    filas = []
    for sec, textos in es_p.items():
        for k, v in textos.items():
            filas.append((f'p|{sec}|{k}', v, en_p.get(sec, {}).get(k, '')))
    for k, v in es_z.items():
        filas.append((f'z|{k}', v, en_z.get(k, '')))
    return filas

filas = plano(PROG)
trozos, actual, palabras = [], [], 0
for f in filas:
    actual.append(f)
    palabras += len(f[1].split())
    if palabras >= 2500:
        trozos.append(actual); actual, palabras = [], 0
if actual:
    trozos.append(actual)
for i, t in enumerate(trozos, 1):
    (SAL / f'{i:02d}.es.json').write_text(json.dumps({k: v for k, v, _ in t}, ensure_ascii=False, indent=0), encoding='utf-8')
    (SAL / f'{i:02d}.en.json').write_text(json.dumps({k: e for k, _, e in t}, ensure_ascii=False, indent=0), encoding='utf-8')
print(len(filas), 'textos en', len(trozos), 'trozos')
