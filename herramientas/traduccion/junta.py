#!/usr/bin/env python3
"""Junta los trozos traducidos de un idioma en sus dos archivos del programa: crates/panel/i18n/<idioma>.json (con sus
secciones) y crates/zero/i18n/<idioma>.json (plano). Solo con trozos que pasen comprueba.py.
Uso: python3 junta.py <idioma> /home/claude/guardiana-av"""
import json, pathlib, sys, subprocess
AQUI = pathlib.Path(__file__).resolve().parent
lang, PROG = sys.argv[1], pathlib.Path(sys.argv[2])
if subprocess.run([sys.executable, str(AQUI / 'comprueba.py'), lang], capture_output=True).returncode:
    sys.exit('no pasa comprueba.py')
panel, zero = {}, {}
for p in sorted((AQUI / lang).glob('[0-9][0-9].json')):
    for k, v in json.loads(p.read_text(encoding='utf-8')).items():
        partes = k.split('|')
        if partes[0] == 'p':
            panel.setdefault(partes[1], {})[partes[2]] = v
        else:
            zero[partes[1]] = v
es_p = json.loads((PROG / 'crates/panel/i18n/es.json').read_text(encoding='utf-8'))
# the same order as Spanish, section by section
orden = {sec: {k: panel[sec][k] for k in textos if k in panel.get(sec, {})} for sec, textos in es_p.items() if sec in panel}
es_z = json.loads((PROG / 'crates/zero/i18n/es.json').read_text(encoding='utf-8'))
orden_z = {k: zero[k] for k in es_z if k in zero}
(PROG / f'crates/panel/i18n/{lang}.json').write_text(json.dumps(orden, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
(PROG / f'crates/zero/i18n/{lang}.json').write_text(json.dumps(orden_z, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
print(lang, 'panel', sum(len(v) for v in orden.values()), 'zero', len(orden_z))
