#!/usr/bin/env python3
"""The installer pictures (WiX, 1.0.18), for GUARDIANA and for GUARDIANA ZERO: the side of the
first and last screens (493x312, one per language: it carries the phrase of every social
profile) and the band of the others (493x58). Navy panel, the white symmetric symbol, the name
centred under it. Rendered by Chromium with the panel's own fonts and saved as 24-bit BMP next
to this file.
Usage: python3 build/wix/imagenes.py <scratch dir for the html and png>"""
import pathlib, sys
from playwright.sync_api import sync_playwright
from PIL import Image
D = pathlib.Path(__file__).resolve().parent
FUENTES = D.parents[1] / 'crates' / 'panel' / 'static' / 'fonts'
SYM = lambda w, trazo='#FFFFFF': f'<svg viewBox="0 0 200 200" width="{w}" height="{w}"><path d="M160.59 69.13 A68 68 0 1 1 130.87 39.41" fill="none" stroke="{trazo}" stroke-width="24"/><circle cx="100" cy="100" r="27" fill="#1F4BFF"/></svg>'
BASE = f'''@font-face{{font-family:Unbounded;font-weight:400 800;src:url({FUENTES}/Unbounded-latin.woff2)}}
@font-face{{font-family:'IBM Plex Mono';font-weight:500;src:url({FUENTES}/IBMPlexMono-500-latin.woff2)}}
*{{box-sizing:border-box}}html,body{{margin:0;background:#fff;overflow:hidden;-webkit-font-smoothing:antialiased}}
.panel{{position:absolute;left:0;top:0;background:#0B1020;
 background-image:radial-gradient(120% 70% at 50% 18%,rgba(31,75,255,.30),rgba(31,75,255,0) 62%),
 linear-gradient(rgba(255,255,255,.035) 1px,transparent 1px),linear-gradient(90deg,rgba(255,255,255,.035) 1px,transparent 1px);
 background-size:100% 100%,16px 16px,16px 16px;background-position:0 0,-1px -1px,-1px -1px}}
.col{{position:absolute;left:0;right:0;display:flex;justify-content:center}}
.marca{{font:700 12px/1 Unbounded;letter-spacing:.2em;padding-left:.2em;color:#fff}}
.zero{{font:700 10px/1 Unbounded;letter-spacing:.32em;padding:5px 6px 5px calc(6px + .32em);color:#fff;background:#1F4BFF;border-radius:999px}}
.web{{font:500 9.5px/1 'IBM Plex Mono';color:#8E9AC6;letter-spacing:.02em}}
.filo{{position:absolute;background:linear-gradient(180deg,#1F4BFF,rgba(31,75,255,0))}}
'''
# The phrase of every social profile (the owner, 12 Oct 2026), in the white that the last screen
# leaves empty between its text (WiX ExitDialog: description down to y=147 px; FatalError to
# 200; UserExit to 213) and the «open the panel» box (from y=253). On the first screen the
# licence box covers it.
# 15 Oct 2026, the owner: «mientras duermes» read as if it only happened at night. It happens
# whenever nobody is watching, so that is what the line says now.
FRASE = {'es': ('«Mira a quién habla tu PC cuando tú no miras.»', 'Haz que te borren.'),
         'en': ('“See who your PC talks to when you’re not looking.”', 'Make them delete you.'),
         'pt': ('«Veja com quem o seu PC fala quando você não olha.»', 'Faça que te apaguem.')}
def dialogo(zero, idioma):
    a, b = FRASE[idioma]
    return f'''<!doctype html><meta charset=utf-8><style>{BASE}
@font-face{{font-family:'IBM Plex Sans';font-weight:500;src:url({FUENTES}/IBMPlexSans-500-latin.woff2)}}
@font-face{{font-family:'IBM Plex Sans';font-weight:600;src:url({FUENTES}/IBMPlexSans-600-latin.woff2)}}
html,body{{width:493px;height:312px}}.panel{{width:164px;height:312px}}
.filo{{left:163px;top:0;width:1px;height:312px}}
.cita{{position:absolute;left:180px;top:208px;width:296px;padding-left:12px;border-left:3px solid #1F4BFF}}
.cita p{{margin:0;font:500 12px/1.4 'IBM Plex Sans';color:#0B1020;white-space:nowrap}}
.cita .b{{font-weight:600;color:#1F4BFF}}</style>
<div class=cita><p>{a}</p><p class=b>{b}</p></div>
<script>document.fonts.ready.then(()=>{{for(const e of document.querySelectorAll('.cita p')){{let t=12;while(e.scrollWidth>282&&t>9){{t-=.25;e.style.fontSize=t+'px'}}}}document.body.dataset.listo=1}})</script>
<div class=panel>
 <div class=col style="top:{70 if zero else 82}px">{SYM(68)}</div>
 <div class=col style="top:{158 if zero else 170}px"><span class=marca>GUARDIANA</span></div>
 {'<div class=col style="top:182px"><span class=zero>ZERO</span></div>' if zero else ''}
 <div class=col style="top:284px"><span class=web>guardianagroup.com</span></div>
</div><div class=filo></div>'''
def banner(zero):
    return f'''<!doctype html><meta charset=utf-8><style>{BASE}
html,body{{width:493px;height:58px}}.panel{{left:auto;right:0;width:82px;height:58px;
 background-image:radial-gradient(90% 90% at 50% 30%,rgba(31,75,255,.32),rgba(31,75,255,0) 70%)}}
.panel .marca{{font-size:6.6px;letter-spacing:.16em;padding-left:.16em}}
.panel .zero{{font-size:6.6px;padding:2.5px 4px 2.5px calc(4px + .32em)}}</style>
<div class=panel>
 <div class=col style="top:{6 if zero else 10}px">{SYM(22 if zero else 26)}</div>
 <div class=col style="top:{32 if zero else 42}px"><span class=marca>GUARDIANA</span></div>
 {'<div class=col style="top:41px"><span class=zero>ZERO</span></div>' if zero else ''}
</div>'''
out = pathlib.Path(sys.argv[1])
with sync_playwright() as p:
    b = p.chromium.launch(executable_path='/opt/pw-browsers/chromium-1194/chrome-linux/chrome')
    trabajos = [('banner', banner(False), 493, 58), ('banner-zero', banner(True), 493, 58)]
    for i in FRASE:
        trabajos += [(f'dialogo-{i}', dialogo(False, i), 493, 312), (f'dialogo-zero-{i}', dialogo(True, i), 493, 312)]
    for nombre, html, w, h in trabajos:
        f = out / f'{nombre}.html'; f.write_text(html)
        pg = b.new_page(viewport={'width': w, 'height': h})
        pg.goto(f.as_uri()); pg.evaluate('document.fonts.ready'); pg.wait_for_timeout(500)
        png = out / f'{nombre}.png'; pg.screenshot(path=str(png)); pg.close()
        Image.open(png).convert('RGB').save(D / f'{nombre}.bmp')
    b.close()
print('ok')
