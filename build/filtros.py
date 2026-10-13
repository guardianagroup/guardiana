#!/usr/bin/env python3
"""ZERO's rule lists: renew them from a copy of uBlockOrigin/uAssets, or read one in clear.

  python3 build/filtros.py --renovar <uAssets folder>   writes crates/zero/data/filtros/*.bin
  python3 build/filtros.py --leer <file.bin>            prints it in clear

The lists are stored encoded (see crates/zero/data/filtros/LEEME.md): the malware lists carry
real malware addresses, and an antivirus that sees them inside a program takes it for one.
"""
import pathlib
import re
import subprocess
import sys

SEMILLA = 0x9E3779B97F4A7C15
DATOS = pathlib.Path(__file__).resolve().parent.parent / 'crates/zero/data/filtros'


def secuencia(n):
    x = SEMILLA
    out = bytearray(n)
    m = 0xFFFFFFFFFFFFFFFF
    for i in range(n):
        x ^= (x << 13) & m
        x ^= x >> 7
        x ^= (x << 17) & m
        out[i] = x & 0xFF
    return out


def codifica(b):
    return bytes(a ^ c for a, c in zip(b, secuencia(len(b))))


RED = [
    ('easylist', 'thirdparties/easylist/easylist.txt', 'EasyList', 'GPL-3.0-or-later / CC BY-SA 3.0', 'https://easylist.to'),
    ('easyprivacy', 'thirdparties/easylist/easyprivacy.txt', 'EasyPrivacy', 'GPL-3.0-or-later / CC BY-SA 3.0', 'https://easylist.to'),
    ('ubo', 'filters/filters.txt', 'uBlock filters', 'GPL-3.0', 'https://github.com/uBlockOrigin/uAssets'),
    ('ubo-privacy', 'filters/privacy.txt', 'uBlock filters – Privacy', 'GPL-3.0', 'https://github.com/uBlockOrigin/uAssets'),
    ('ubo-badware', 'filters/badware.txt', 'uBlock filters – Badware risks', 'GPL-3.0', 'https://github.com/uBlockOrigin/uAssets'),
    ('ubo-unbreak', 'filters/unbreak.txt', 'uBlock filters – Unbreak', 'GPL-3.0', 'https://github.com/uBlockOrigin/uAssets'),
    ('urlhaus', 'thirdparties/urlhaus-filter/urlhaus-filter-online.txt', 'Online Malicious URL Blocklist (URLhaus)', 'CC0 / MIT (malware-filter)', 'https://gitlab.com/malware-filter/urlhaus-filter'),
]
COSMETICA = re.compile(r'#[@?$%]?#|#\+js|#@\$#|#@\?#')
PROCEDIMIENTO = re.compile(r':-abp|:has-text|:upward|:remove|:style\(|:xpath|:matches-|:watch-|:min-text|:others|:shadow|:matches-path|:if\(|:if-not|:nth-ancestor|:contains\(|:spath|\{')


def renueva(ua):
    ua = pathlib.Path(ua)
    commit = subprocess.run(['git', '-C', str(ua), 'rev-parse', 'HEAD'], capture_output=True, text=True).stdout.strip()[:12]
    for nombre, src, titulo, lic, web in RED:
        lineas = (ua / src).read_text(encoding='utf-8').splitlines()
        ver = [l for l in lineas[:20] if l.startswith(('! Version', '! Updated', '! Last modified'))][:1]
        reglas = [s.strip() for s in lineas if s.strip() and not s.strip().startswith(('!', '[')) and not COSMETICA.search(s)]
        cab = [f'! {titulo}', f'! Origen: {web} (copia de uBlockOrigin/uAssets, commit {commit})', f'! Licencia: {lic}'] + ver + ['! Solo las reglas de red: sin comentarios ni reglas cosméticas.']
        (DATOS / f'{nombre}.bin').write_bytes(codifica(('\n'.join(cab + reglas) + '\n').encode()))
        print(nombre, len(reglas))
    cos = []
    for src in ('thirdparties/easylist/easylist.txt', 'filters/filters.txt'):
        for l in (ua / src).read_text(encoding='utf-8').splitlines():
            s = l.strip()
            m = re.match(r'^([^#]*)(##|#@#)(.+)$', s)
            if not s or s.startswith('!') or not m:
                continue
            sel = m.group(3)
            if sel.startswith(('+js', '^')) or PROCEDIMIENTO.search(sel):
                continue
            cos.append(s)
    cab = ['! Reglas cosméticas (ocultar elementos) de EasyList y uBlock filters', f'! Origen: https://easylist.to y https://github.com/uBlockOrigin/uAssets (commit {commit})', '! Licencia: GPL-3.0-or-later / CC BY-SA 3.0 (EasyList); GPL-3.0 (uBlock filters)', '! Solo «##» y «#@#» con selectores CSS normales: sin scriptlets ni selectores de procedimiento.']
    (DATOS / 'cosmeticos.bin').write_bytes(codifica(('\n'.join(cab + cos) + '\n').encode()))
    print('cosmeticos', len(cos))


if __name__ == '__main__':
    if len(sys.argv) == 3 and sys.argv[1] == '--leer':
        sys.stdout.write(codifica(pathlib.Path(sys.argv[2]).read_bytes()).decode())
    elif len(sys.argv) == 3 and sys.argv[1] == '--renovar':
        renueva(sys.argv[2])
    else:
        sys.exit(__doc__)
