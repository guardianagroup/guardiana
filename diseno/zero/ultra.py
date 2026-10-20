# ULTRA (round 7b): reinforced private browsing through Tor. Design prototype; honest about what it does and does not do.
ULTRA_CSS = r'''/* ULTRA: a button in the bar, a double luminous fillet on the glass, a subtle line under the bar and an honest panel. No red. */
#ultra{display:flex;align-items:center;gap:7px;height:39px;padding:0 11px;border-radius:11px;border:1px solid rgba(156,195,228,.16);background:transparent;color:#a9c0d2;font:10px Consolas,monospace;letter-spacing:.18em;transition:background .3s,border-color .3s,color .3s,box-shadow .3s}
#ultra .anillos{width:16px;height:16px;border-radius:50%;border:1px solid currentColor;box-shadow:inset 0 0 0 3px #0b121b,inset 0 0 0 4px currentColor}
#ultra:hover{color:#F4F6F8;border-color:rgba(156,195,228,.4)}
#ultra[aria-pressed=true]{color:#F4F6F8;border-color:rgba(244,246,248,.55);background:rgba(156,195,228,.14);box-shadow:0 0 16px rgba(156,195,228,.35)}
#stage.ultra .lens{outline:1px solid rgba(244,246,248,.42);outline-offset:6px}
#stage.ultra.open .lens{outline-offset:4px}
#stage.ultra:before{content:"";position:absolute;inset:0;z-index:3;pointer-events:none;background:radial-gradient(ellipse 70% 62% at 50% 48%,transparent 50%,rgba(0,0,0,.5));animation:ultraIn .6s ease-out}
@keyframes ultraIn{from{opacity:0}to{opacity:1}}
.ultraline{position:absolute;z-index:4;top:531px;left:0;right:0;text-align:center;font:10px Consolas,monospace;letter-spacing:.16em;color:#8aa5b9;opacity:0;pointer-events:none;transition:opacity .4s}
.ultraline b{color:#F4F6F8;font-weight:400}
.ultraline button{all:unset;cursor:pointer;margin-left:14px;color:#cfe0ec;border-bottom:1px solid rgba(156,195,228,.4);pointer-events:auto}
.ultraline button:focus-visible{outline:2px solid #9CC3E4;outline-offset:3px}
#stage.ultra .ultraline{opacity:1}
#stage.ultra .hint{opacity:0}
#stage.open .ultraline,#stage.intro .ultraline{opacity:0;pointer-events:none}
#stage.open .ultraline button{pointer-events:none}
.upanel{position:absolute;z-index:45;left:390px;top:562px;width:660px;padding:18px 22px 16px;border-radius:18px;border:1px solid rgba(156,195,228,.28);background:linear-gradient(180deg,rgba(14,21,31,.96),rgba(7,11,17,.98));box-shadow:0 26px 70px #000c;color:#d7e5ef;font:12.5px/1.55 "Segoe UI",Inter,Arial,sans-serif;opacity:0;visibility:hidden;transform:translateY(8px);transition:opacity .3s,transform .3s,visibility .3s}
.upanel.show{opacity:1;visibility:visible;transform:none}
#stage.open .upanel{top:96px;left:420px}
.upanel h3{margin:0 0 4px;font:600 13px "Segoe UI",Inter,Arial,sans-serif;letter-spacing:.24em;color:#F4F6F8}
.upanel h3 span{font-weight:400;letter-spacing:.04em;color:#8fa9bd;margin-left:10px}
.upanel .lead{margin:0 0 12px;color:#b9cddb}
.ucols{display:grid;grid-template-columns:1.25fr 1fr;gap:22px}
.ucols h4{margin:0 0 6px;font:9.5px Consolas,monospace;letter-spacing:.2em;color:#8fa9bd;font-weight:400}
.ucols ul{margin:0;padding:0;list-style:none;display:grid;gap:5px}
.ucols li{padding-left:16px;position:relative;color:#d7e5ef}
.ucols li:before{content:"";position:absolute;left:0;top:.62em;width:7px;height:7px;border-radius:50%;border:1px solid #9CC3E4}
.ucols .no li:before{border-color:#6f889c;background:none;width:9px;height:1px;border:0;border-top:1px solid #6f889c;top:.75em}
.ucols .no li{color:#a8bccb}
.univ{margin-top:14px;display:flex;gap:6px}
.univ button{flex:1;padding:8px 10px;border-radius:10px;border:1px solid rgba(156,195,228,.2);background:rgba(156,195,228,.04);color:#cfe0ec;font:11.5px "Segoe UI",Inter,Arial,sans-serif;text-align:left}
.univ button small{display:block;font:9.5px Consolas,monospace;color:#7d97ab;margin-top:3px;letter-spacing:.02em}
.univ button[aria-pressed=true]{border-color:#F4F6F8;background:rgba(156,195,228,.13)}
.upanel .ufoot{margin-top:12px;font:9.5px Consolas,monospace;letter-spacing:.05em;color:#6c8597;line-height:1.6}
.mnota{display:none;margin:0 14px 10px;padding:8px 10px;border-radius:10px;border:1px solid rgba(244,246,248,.25);font:10px Consolas,monospace;letter-spacing:.05em;color:#cfe0ec}
#stage.ultra .mnota{display:block}
'''
rep("\n.director{position:fixed", "\n" + ULTRA_CSS + ".director{position:fixed")
rep('<button type="button" id="searcher"', '<button type="button" id="ultra" aria-pressed="false" title="ULTRA: navegación reforzada mediante Tor"><i class="anillos" aria-hidden="true"></i>ULTRA</button><button type="button" id="searcher"')
rep('<div class="hint">', '<div class="ultraline" aria-live="polite"><b>◎ ULTRA</b> · Ruta Tor · Sesión separada · Nivel: <span id="ulvl">Más seguro</span><button type="button" id="uqueEs">¿Qué te protege?</button></div>'
    '<div class="upanel" id="upanel" role="dialog" aria-label="Qué es ULTRA"><h3>ULTRA<span>Navegación reforzada mediante Tor</span></h3>'
    '<p class="lead">Tus páginas viajan por la red Tor, saltando por tres repetidores: la web ve el último, no a ti. ZERO abre además una sesión aparte que se borra al cerrar y hace que tu navegador se parezca al de muchas otras personas.</p>'
    '<div class="ucols"><div><h4>LO QUE HACE</h4><ul>'
    '<li>Ruta Tor: tu dirección IP queda oculta para la web.</li>'
    '<li>DNS por Tor: tu proveedor no ve qué direcciones consultas.</li>'
    '<li>Sesión separada: sin historial, cookies ni caché compartidos; todo se borra al cerrar.</li>'
    '<li>Sin fugas: WebRTC y conexiones directas bloqueadas; si Tor falla, no se navega.</li>'
    '<li>Huella uniforme: ventana por bloques, hora UTC, idioma y agente comunes.</li>'
    '<li>Rastreadores, cookies de terceros y parámetros de seguimiento, fuera.</li>'
    '<li>Buscador centrado en privacidad por defecto.</li></ul></div>'
    '<div class="no"><h4>LO QUE NO PUEDE HACER</h4><ul>'
    '<li>Si inicias sesión en una cuenta, ese servicio sabe quién eres.</li>'
    '<li>Lo que descargues y abras fuera de ZERO puede conectarse sin protección.</li>'
    '<li>Tu proveedor de internet puede ver que usas Tor, aunque no qué visitas.</li>'
    '<li>Ninguna herramienta garantiza el anonimato total.</li></ul></div></div>'
    '<div class="univ" role="group" aria-label="Nivel de seguridad">'
    '<button type="button" data-l="Estándar">Estándar<small>Tor y sesión aparte; JavaScript activo</small></button>'
    '<button type="button" data-l="Más seguro" aria-pressed="true">Más seguro<small>Sin JavaScript en webs sin cifrar; menos multimedia</small></button>'
    '<button type="button" data-l="Máxima seguridad">Máxima seguridad<small>Sin JavaScript; lo mínimo para identificarte</small></button></div>'
    '<div class="ufoot">Diseño en prueba: en esta demostración no hay conexión Tor real. ULTRA se activará en ZERO solo cuando supere pruebas independientes de fugas de IP, DNS, WebRTC, almacenamiento y huella digital.</div></div>'
    '<div class="hint">')
rep("<div class=\"mfav\" id=\"mfav\"", "<div class=\"mnota\">En ULTRA se usa un buscador del grupo Privacidad. Puedes cambiarlo, a conciencia.</div><div class=\"mfav\" id=\"mfav\"")

ULTRA_JS = r'''
// ULTRA: switching on keeps the previous engine to restore it later and moves to a privacy-focused one.
let ultra=false,antesUltra=null;const upanel=$('#upanel');
function panelUltra(on){upanel.classList.toggle('show',on)}
function ponUltra(on){ultra=on;stage.classList.toggle('ultra',on);$('#ultra').setAttribute('aria-pressed',String(on));
  if(on){if(engOf(selected)[0]!=='PRIVACIDAD'||selected==='Ecosia'||selected==='Kagi'){antesUltra=selected;selected='DuckDuckGo';pintaBoton()}panelUltra(true);clearTimeout(ponUltra.t);ponUltra.t=setTimeout(()=>panelUltra(false),9000)}
  else{panelUltra(false);if(antesUltra){selected=antesUltra;antesUltra=null;pintaBoton()}}
  const L=$('#lens');L.classList.remove('elegido');void L.offsetWidth;L.classList.add('elegido');pulse=Math.max(pulse,1.1)}
$('#ultra').onclick=()=>ponUltra(!ultra);
$('#uqueEs').onclick=()=>{clearTimeout(ponUltra.t);panelUltra(!upanel.classList.contains('show'))};
upanel.querySelectorAll('.univ button').forEach(b=>b.onclick=()=>{upanel.querySelectorAll('.univ button').forEach(x=>x.setAttribute('aria-pressed',String(x===b)));$('#ulvl').textContent=b.dataset.l;clearTimeout(ponUltra.t)});
document.addEventListener('pointerdown',e=>{if(upanel.classList.contains('show')&&!upanel.contains(e.target)&&e.target.id!=='uqueEs'&&!$('#ultra').contains(e.target))panelUltra(false)});
document.addEventListener('keydown',e=>{if(e.key==='Escape')panelUltra(false)});
'''
rep("$('#searcher').onclick=()=>abre(!menu.classList.contains('show'));", "$('#searcher').onclick=()=>abre(!menu.classList.contains('show'));" + ULTRA_JS)

# ---- geometry fix: the panel keeps the bar's width and edges, compact, always inside the stage ----
for a, b in [
 (".upanel{position:absolute;z-index:45;left:390px;top:562px;width:660px;padding:18px 22px 16px;",
  ".upanel{position:absolute;z-index:45;left:307px;top:556px;width:826px;max-height:318px;overflow:auto;padding:14px 20px 12px;"),
 ("#stage.open .upanel{top:96px;left:420px}","#stage.open .upanel{top:92px;left:25px;width:826px}"),
 (".upanel h3{margin:0 0 4px;",".upanel .uhead{display:flex;align-items:center;justify-content:space-between;gap:14px;margin-bottom:6px}.upanel h3{margin:0;"),
 (".upanel .lead{margin:0 0 12px;color:#b9cddb}",".upanel .lead{margin:0 0 10px;color:#b9cddb;font-size:12px}"),
 (".ucols{display:grid;grid-template-columns:1.25fr 1fr;gap:22px}",".ucols{display:grid;grid-template-columns:1.3fr 1fr;gap:26px;font-size:11.5px;line-height:1.45}"),
 (".ucols ul{margin:0;padding:0;list-style:none;display:grid;gap:5px}",".ucols ul{margin:0;padding:0;list-style:none;display:grid;gap:3px}"),
 (".univ{margin-top:14px;display:flex;gap:6px}",".univ{display:flex;gap:4px;flex:none}"),
 (".univ button{flex:1;padding:8px 10px;border-radius:10px;border:1px solid rgba(156,195,228,.2);background:rgba(156,195,228,.04);color:#cfe0ec;font:11.5px \"Segoe UI\",Inter,Arial,sans-serif;text-align:left}",
  ".univ button{padding:5px 10px;border-radius:999px;border:1px solid rgba(156,195,228,.2);background:rgba(156,195,228,.04);color:#cfe0ec;font:11px \"Segoe UI\",Inter,Arial,sans-serif}"),
 (".univ button small{display:block;font:9.5px Consolas,monospace;color:#7d97ab;margin-top:3px;letter-spacing:.02em}",".univ button small{display:none}.univtxt{margin:8px 0 0;font:10px Consolas,monospace;color:#8ea6b8;letter-spacing:.03em}"),
 (".upanel .ufoot{margin-top:12px;",".upanel .ufoot{margin-top:8px;"),
]:
    rep(a, b)
import re as _re
_m = _re.search(r'<div class="univ" role="group" aria-label="Nivel de seguridad">.*?</div>', s)
assert _m
_lv = ('<div class="univ" role="group" aria-label="Nivel de seguridad">'
 '<button type="button" data-l="Estándar" data-t="Tor y sesión aparte; JavaScript activo.">Estándar</button>'
 '<button type="button" data-l="Más seguro" data-t="Sin JavaScript en webs sin cifrar y menos multimedia." aria-pressed="true">Más seguro</button>'
 '<button type="button" data-l="Máxima seguridad" data-t="Sin JavaScript: lo mínimo para identificarte.">Máxima seguridad</button></div>')
s = s[:_m.start()] + '<p class="univtxt" id="univtxt">NIVEL MÁS SEGURO · Sin JavaScript en webs sin cifrar y menos multimedia.</p>' + s[_m.end():]
rep('<h3>ULTRA<span>Navegación reforzada mediante Tor</span></h3>', '<div class="uhead"><h3>ULTRA<span>Navegación reforzada mediante Tor</span></h3>' + _lv + '</div>')
rep('Tus páginas viajan por la red Tor, saltando por tres repetidores: la web ve el último, no a ti. ZERO abre además una sesión aparte que se borra al cerrar y hace que tu navegador se parezca al de muchas otras personas.',
    'Tus páginas viajan por la red Tor saltando por tres repetidores: la web ve el último, no a ti. Tu sesión va aparte y se borra al cerrar.')
rep('Diseño en prueba: en esta demostración no hay conexión Tor real. ULTRA se activará en ZERO solo cuando supere pruebas independientes de fugas de IP, DNS, WebRTC, almacenamiento y huella digital.',
    'Diseño en prueba: aquí no hay conexión Tor real. ULTRA llegará a ZERO cuando supere pruebas independientes de fugas.')
rep("$('#ulvl').textContent=b.dataset.l;", "$('#ulvl').textContent=b.dataset.l;$('#univtxt').textContent='NIVEL '+b.dataset.l.toUpperCase()+' · '+b.dataset.t;")
