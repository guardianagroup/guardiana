#!/usr/bin/env python3
"""The installer pictures (WiX, 1.0.18; new logo 17 Oct 2026), for GUARDIANA and for GUARDIANA ZERO: the side of the
first and last screens (493x312, one per language: it carries the phrase of every social
profile) and the band of the others (493x58). Near-black panel, the padlock-G symbol, the name
centred under it. Since 3 Nov 2026 also the page shown while GUARDIANA installs (instalando-<es|en|pt>.bmp, 493x312: its
title is drawn here, in the language of each installer, read from loc-*.wxl; the step, the bar and the lines are the dialog's
own controls, see InstalandoDlg in guardiana.wxs). Rendered by Chromium with the panel's own fonts and saved as 24-bit BMP
next to this file.
Usage: python3 build/wix/imagenes.py <scratch dir for the html and png> [name ...]
With names (banner, banner-zero, dialogo-es, ..., instalando-es) only those are drawn again: a newer Chromium draws the
letters a hair differently, and pictures nobody changed should not change."""
import glob, os, pathlib, re, sys
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
if not _SCHIBSTED.is_file():
    _SCHIBSTED = FUENTES / 'SchibstedGrotesk-var.woff'  # the same file, carried by the panel since 1.0.4x
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
# 17 Oct 2026: the phrase of 12 Oct («Mira a quién habla tu PC cuando tú no miras» / «Haz que te
# borren») was out of date for the owner. Chosen by the committee (Claude, ChatGPT, Grok): what
# GUARDIANA shows that nobody else does, and who decides.
FRASE = {'es': ('Tu PC, tu IA y tu antivirus, a la vista.', 'Tú decides qué se corta.'),
         'en': ('Your PC, your AI and your antivirus, in plain view.', 'You decide what gets cut.'),
         'pt': ('Seu PC, sua IA e seu antivírus, à vista.', 'Você decide o que bloquear.')}
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
# The page while GUARDIANA installs (3 Nov 2026, the owner: «que salga la pantalla de un instalador bien bonita… una página
# entera de Guardiana se está instalando… algo muy profesional»). InstalandoDlg in guardiana.wxs puts this picture over the
# whole dialog above its button band (370x234 installer units, 493x312 px at 100 %) and its own words over the empty dark
# lower part, from y=150 px: the title, the step, the bar and three lines. Like the program's start screen: dark, a few stars
# and a calm sphere with its network and its orbit, the symbol and the name at the top left and the web at the foot. All
# drawn here (no photograph, no logo of anyone else), at twice the size and reduced by the browser so the dots stay smooth.
# The stars are fixed (a seeded random), so drawing it again gives the same picture.
DIBUJO = r'''
const c=document.getElementById('c'),x=c.getContext('2d');x.scale(2,2);
const W=493,H=312,CX=386,CY=84,R=64,PI=Math.PI;
let semilla=7;const azar=()=>(semilla=semilla*16807%2147483647)/2147483647;
x.fillStyle='#06080C';x.fillRect(0,0,W,H);
let g=x.createRadialGradient(CX,CY,0,CX,CY,240);g.addColorStop(0,'rgba(44,86,140,.26)');g.addColorStop(.5,'rgba(22,44,76,.10)');g.addColorStop(1,'rgba(6,8,12,0)');
x.fillStyle=g;x.fillRect(0,0,W,H);
// stars: never behind the words (the name at the top, the dialog's words below y=142) nor on the sphere
for(let i=0,n=0;i<400&&n<64;i++){const px=azar()*W,py=azar()*H,r=.35+azar()*.75,a=.12+azar()*.42;
  if(py>142||(px<215&&py<54)||Math.hypot(px-CX,py-CY)<R+26)continue;
  x.beginPath();x.arc(px,py,r,0,7);x.fillStyle=`rgba(207,224,240,${a.toFixed(3)})`;x.fill();n++;}
const giro=(v,ay,ax)=>{let[X,Y,Z]=v;const cy=Math.cos(ay),sy=Math.sin(ay);[X,Z]=[X*cy+Z*sy,-X*sy+Z*cy];
  const cx=Math.cos(ax),sx=Math.sin(ax);[Y,Z]=[Y*cx-Z*sx,Y*sx+Z*cx];return[X,Y,Z];};
const punto=(la,lo)=>giro([Math.cos(la)*Math.sin(lo),Math.sin(la),Math.cos(la)*Math.cos(lo)],-.45,.32);
function orbita(desde,hasta,alfa){x.save();x.translate(CX,CY);x.rotate(-.30);x.strokeStyle=`rgba(156,195,228,${alfa})`;x.lineWidth=1;
  x.beginPath();x.ellipse(0,0,R*1.62,R*.42,0,desde,hasta);x.stroke();x.restore();}
orbita(PI,2*PI,.20);
let a=x.createRadialGradient(CX,CY,R*.92,CX,CY,R*1.34);a.addColorStop(0,'rgba(120,170,230,.30)');a.addColorStop(1,'rgba(120,170,230,0)');
x.fillStyle=a;x.beginPath();x.arc(CX,CY,R*1.34,0,7);x.fill();
let b=x.createRadialGradient(CX-R*.38,CY-R*.42,R*.05,CX,CY,R);b.addColorStop(0,'#17324F');b.addColorStop(.55,'#0C1A2E');b.addColorStop(1,'#070D18');
x.fillStyle=b;x.beginPath();x.arc(CX,CY,R,0,7);x.fill();
x.save();x.beginPath();x.arc(CX,CY,R-.3,0,7);x.clip();
function mota(la,lo){const v=punto(la*PI/180,lo*PI/180);if(v[2]<=.02)return;
  x.beginPath();x.arc(CX+R*v[0],CY-R*v[1],.6+.28*v[2],0,7);x.fillStyle=`rgba(156,195,228,${(.08+.52*v[2]*v[2]).toFixed(3)})`;x.fill();}
for(let la=-75;la<=75;la+=15)for(let lo=0;lo<360;lo+=5)mota(la,lo);
for(let lo=0;lo<360;lo+=30)for(let la=-86;la<=86;la+=4)mota(la,lo);
x.restore();
x.save();x.shadowColor='rgba(110,165,230,.8)';x.shadowBlur=16;x.strokeStyle='rgba(170,205,235,.62)';x.lineWidth=1.1;
x.beginPath();x.arc(CX,CY,R,0,7);x.stroke();x.restore();
// the network: this PC (the brighter node, near the middle) and the places it talks to
const nodos=[[-.55,.42],[-.16,.64],[.32,.47],[.60,.06],[.05,.08],[-.47,-.36],[.36,-.58]].map(([u,v])=>[u,v,Math.sqrt(1-u*u-v*v)]);
function arco(p,q){const d=Math.acos(Math.min(1,p[0]*q[0]+p[1]*q[1]+p[2]*q[2]));x.beginPath();
  for(let i=0;i<=48;i++){const t=i/48,s1=Math.sin((1-t)*d)/Math.sin(d),s2=Math.sin(t*d)/Math.sin(d),L=1+.17*Math.sin(PI*t)*d;
    const px=CX+R*L*(p[0]*s1+q[0]*s2),py=CY-R*L*(p[1]*s1+q[1]*s2);i?x.lineTo(px,py):x.moveTo(px,py);}x.stroke();}
x.save();x.lineWidth=1;x.strokeStyle='rgba(156,195,228,.55)';x.shadowColor='rgba(156,195,228,.5)';x.shadowBlur=4;
for(const[i,j]of[[4,0],[4,2],[4,3],[4,5],[4,6],[1,2]])arco(nodos[i],nodos[j]);x.restore();
nodos.forEach((n,i)=>{const px=CX+R*n[0],py=CY-R*n[1];x.save();x.shadowColor='rgba(156,195,228,.95)';x.shadowBlur=i==4?10:6;
  x.fillStyle='#E8ECF1';x.beginPath();x.arc(px,py,i==4?2.3:1.5,0,7);x.fill();x.restore();
  if(i==4){x.strokeStyle='rgba(156,195,228,.45)';x.lineWidth=1;x.beginPath();x.arc(px,py,5.5,0,7);x.stroke();}});
orbita(0,PI,.34);
const t=.62*PI,ox=R*1.62*Math.cos(t),oy=R*.42*Math.sin(t),cr=Math.cos(-.30),sr=Math.sin(-.30);
x.save();x.shadowColor='rgba(156,195,228,.95)';x.shadowBlur=8;x.fillStyle='#E8ECF1';x.beginPath();
x.arc(CX+ox*cr-oy*sr,CY+ox*sr+oy*cr,1.7,0,7);x.fill();x.restore();
'''
# 5 Nov 2026: the title is drawn in the picture, in each installer's language. As a text control of the dialog (Segoe UI
# Semibold 16, a light TextStyle) Windows Installer drew it in black on the dark picture, unreadable (photos of instalador.yml,
# run 42042498335), while the smaller lines kept their colour. Its words stay in loc-*.wxl (InstalandoTitulo), read here.
CULTURA = {'es': 'es-ES', 'en': 'en-US', 'pt': 'pt-BR'}
def titulo(idioma):
    t = (D / f'loc-{CULTURA[idioma]}.wxl').read_text(encoding='utf-8')
    m = re.search(r'<String Id="InstalandoTitulo" Value="([^"]+)"', t)
    if not m:
        sys.exit(f'loc-{CULTURA[idioma]}.wxl: sin InstalandoTitulo')
    return m.group(1)
def instalando(idioma):
    return f'''<!doctype html><meta charset=utf-8><style>{BASE}
@font-face{{font-family:'IBM Plex Sans';font-weight:600;src:url({FUENTES}/IBMPlexSans-600-latin.woff2)}}
html,body{{width:493px;height:312px;background:#06080C}}
canvas{{position:absolute;left:0;top:0;width:493px;height:312px}}
.cab{{position:absolute;left:24px;top:20px;display:flex;align-items:center;gap:9px}}
.cab .marca{{font-size:12.5px;letter-spacing:.24em;padding-left:0}}
.titulo{{position:absolute;left:24px;top:153px;width:445px;margin:0;font:600 21px/1.2 'IBM Plex Sans';color:#E8ECF1;white-space:nowrap}}
.pie{{position:absolute;right:24px;top:292px}}</style>
<canvas id=c width=986 height=624></canvas>
<div class=cab>{SYM(26)}<span class=marca>GUARDIANA</span></div>
<p class=titulo>{titulo(idioma)}</p>
<div class=pie><span class=web>guardianagroup.com</span></div>
<script>{DIBUJO}
document.fonts.ready.then(()=>{{const e=document.querySelector('.titulo');let t=21;while(e.scrollWidth>445&&t>15){{t-=.25;e.style.fontSize=t+'px'}}}});</script>'''
def chromium():
    """Playwright's Chromium wherever this machine keeps it (chromium-1194 until Oct 2026, chromium-1243 after)."""
    if os.environ.get('CHROMIUM'):
        return os.environ['CHROMIUM']
    hay = sorted(glob.glob('/opt/pw-browsers/chromium-*/chrome-linux*/chrome'))
    return hay[-1] if hay else None
out = pathlib.Path(sys.argv[1])
pedidos = set(sys.argv[2:])
with sync_playwright() as p:
    ruta = chromium()
    b = p.chromium.launch(executable_path=ruta) if ruta else p.chromium.launch()
    trabajos = [('banner', banner(False), 493, 58), ('banner-zero', banner(True), 493, 58)]
    for i in FRASE:
        trabajos += [(f'dialogo-{i}', dialogo(False, i), 493, 312), (f'dialogo-zero-{i}', dialogo(True, i), 493, 312)]
    trabajos += [(f'instalando-{i}', instalando(i), 493, 312) for i in FRASE]
    for nombre, html, w, h in trabajos:
        if pedidos and nombre not in pedidos:
            continue
        f = out / f'{nombre}.html'; f.write_text(html)
        pg = b.new_page(viewport={'width': w, 'height': h})
        pg.goto(f.as_uri()); pg.evaluate('document.fonts.ready'); pg.wait_for_timeout(500)
        png = out / f'{nombre}.png'; pg.screenshot(path=str(png)); pg.close()
        Image.open(png).convert('RGB').save(D / f'{nombre}.bmp')
    b.close()
print('ok')
