#!/usr/bin/env python3
"""The installer pictures (WiX, 1.0.18; new logo 17 Oct 2026), for GUARDIANA and for GUARDIANA ZERO: the side of the
first and last screens (493x312, one per language: it carries the phrase of every social
profile) and the band of the others (493x58). Near-black panel, the padlock-G symbol, the name
centred under it. Rendered by Chromium with the panel's own fonts and saved as 24-bit BMP next
to this file.
Usage: python3 build/wix/imagenes.py <scratch dir for the html and png>"""
import pathlib, sys
from playwright.sync_api import sync_playwright
from PIL import Image
D = pathlib.Path(__file__).resolve().parent
FUENTES = D.parents[1] / 'crates' / 'panel' / 'static' / 'fonts'
# The brand symbol since 17 Oct 2026 (the owner chose it): a padlock whose body carries a square
# G cut out, the shackle in white and the body in steel blue. Same geometry as
# guardianagroup.com/marca/guardiana-simbolo.svg.
_n = iter(range(1, 99))
def SYM(w):
    m = f'g{next(_n)}'
    return (f'<svg viewBox="0 0 100 100" width="{w}" height="{w}"><defs><mask id="{m}"><rect width="100" height="100" fill="#fff"/>'
            '<path d="M63 58.29 V55.07 Q63 54 58.71 54 H44.15 Q37 54 37 61.15 V72.85 Q37 80 44.15 80 H58.71 Q63 80 63 75.71 V67 H51" fill="none" stroke="#000" stroke-width="7.5"/></mask></defs>'
            '<path d="M31 44 V31 A19 19 0 0 1 69 31 V44" fill="none" stroke="#E8ECF1" stroke-width="9"/>'
            f'<rect x="17" y="42" width="66" height="50" rx="13" fill="#9CC3E4" mask="url(#{m})"/></svg>')
# The name in the letter of the brand (Schibsted Grotesk, OFL), from the web's copy when it is
# next to this checkout; otherwise the panel's IBM Plex Sans.
_SCHIBSTED = D.parents[2] / 'guardianagroup.com' / 'fonts' / 'SchibstedGrotesk-var.woff'
MARCA_FUENTE = (f"@font-face{{font-family:Marca;font-weight:400 900;src:url({_SCHIBSTED})}}" if _SCHIBSTED.is_file()
                else f"@font-face{{font-family:Marca;font-weight:600;src:url({FUENTES}/IBMPlexSans-600-latin.woff2)}}")
BASE = MARCA_FUENTE + f'''
@font-face{{font-family:'IBM Plex Mono';font-weight:500;src:url({FUENTES}/IBMPlexMono-500-latin.woff2)}}
*{{box-sizing:border-box}}html,body{{margin:0;background:#fff;overflow:hidden;-webkit-font-smoothing:antialiased}}
.panel{{position:absolute;left:0;top:0;background:#06080C}}
.col{{position:absolute;left:0;right:0;display:flex;justify-content:center}}
.marca{{font:600 13px/1 Marca;letter-spacing:.12em;padding-left:.12em;color:#E8ECF1}}
.grupo{{font:600 7px/1 Marca;letter-spacing:.42em;padding-left:.42em;color:#E8ECF1;opacity:.72}}
.zero{{font:700 9.5px/1 Marca;letter-spacing:.32em;padding:5px 6px 5px calc(6px + .32em);color:#06080C;background:#9CC3E4;border-radius:999px}}
.web{{font:500 9.5px/1 'IBM Plex Mono';color:#8E99A6;letter-spacing:.02em}}
.filo{{position:absolute;background:#1E2631}}
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
.cita{{position:absolute;left:180px;top:208px;width:296px;padding-left:12px;border-left:3px solid #3E6E96}}
.cita p{{margin:0;font:500 12px/1.4 'IBM Plex Sans';color:#0B1020;white-space:nowrap}}
.cita .b{{font-weight:600;color:#2F5C84}}</style>
<div class=cita><p>{a}</p><p class=b>{b}</p></div>
<script>document.fonts.ready.then(()=>{{for(const e of document.querySelectorAll('.cita p')){{let t=12;while(e.scrollWidth>282&&t>9){{t-=.25;e.style.fontSize=t+'px'}}}}document.body.dataset.listo=1}})</script>
<div class=panel>
 <div class=col style="top:{66 if zero else 78}px">{SYM(76)}</div>
 <div class=col style="top:{160 if zero else 172}px"><span class=marca>GUARDIANA</span></div>
 <div class=col style="top:{179 if zero else 191}px"><span class=grupo>GROUP</span></div>
 {'<div class=col style="top:202px"><span class=zero>ZERO</span></div>' if zero else ''}
 <div class=col style="top:284px"><span class=web>guardianagroup.com</span></div>
</div><div class=filo></div>'''
def banner(zero):
    return f'''<!doctype html><meta charset=utf-8><style>{BASE}
html,body{{width:493px;height:58px}}.panel{{left:auto;right:0;width:82px;height:58px}}
.panel .marca{{font-size:7.4px;letter-spacing:.1em;padding-left:.1em}}
.panel .zero{{font-size:6.6px;padding:2.5px 4px 2.5px calc(4px + .32em)}}</style>
<div class=panel>
 <div class=col style="top:{5 if zero else 8}px">{SYM(24 if zero else 30)}</div>
 <div class=col style="top:{31 if zero else 41}px"><span class=marca>GUARDIANA</span></div>
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
