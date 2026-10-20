# The search-engine selector (round 7): Google by default; a glass launcher with every major engine.
SEL_CSS = r'''/* Engine selector: a glass launcher (search inside, favourites and recents, four groups, monograms instead of logos) */
#searcher{display:flex;align-items:center;gap:8px;min-width:132px;padding:6px 10px 6px 6px;font:12px "Segoe UI",Inter,Arial,sans-serif;letter-spacing:.01em;color:#e6eef5}
#searcher .mono{width:24px;height:24px;font-size:10px}
.mono{flex:0 0 auto;display:inline-grid;place-items:center;width:30px;height:30px;border-radius:50%;border:1px solid rgba(156,195,228,.38);background:radial-gradient(circle at 35% 30%,rgba(244,246,248,.16),rgba(156,195,228,.05) 60%,transparent);color:#dbe8f2;font:600 11px "Segoe UI",Inter,Arial,sans-serif;letter-spacing:.02em;font-style:normal}
.menu{left:693px;top:500px;width:440px;max-height:392px;padding:0;display:flex;flex-direction:column;overflow:hidden;border-radius:20px;border:1px solid rgba(156,195,228,.34);background:linear-gradient(180deg,rgba(16,24,34,.97),rgba(7,11,17,.985));box-shadow:0 30px 80px #000d,inset 0 1px 0 rgba(244,246,248,.12);transform:translateY(-8px) scale(.985);transition:opacity .26s,transform .26s cubic-bezier(.2,.85,.2,1),visibility .26s}
#stage.open .menu{top:90px;left:667px}
.menu.show{transition:opacity .26s,transform .26s cubic-bezier(.2,.85,.2,1),visibility 0s}
.mhead{padding:16px 18px 10px;font:10px Consolas,monospace;letter-spacing:.2em;color:#8fa9bd;display:flex;justify-content:space-between}
.mhead b{font-weight:400;color:#cfe0ec}
.mfind{margin:0 14px 10px;height:38px;border-radius:12px;border:1px solid rgba(156,195,228,.22);background:rgba(156,195,228,.06);color:#F4F6F8;padding:0 12px 0 34px;font:13px "Segoe UI",Inter,Arial,sans-serif;outline:none;background-image:radial-gradient(circle at 17px 50%,transparent 5px,rgba(156,195,228,.7) 5.5px,rgba(156,195,228,.7) 6.6px,transparent 7px)}
.mfind:focus{border-color:rgba(156,195,228,.6)}
.mfav{display:flex;gap:6px;flex-wrap:wrap;padding:0 14px 10px}
.mfav button{display:flex;align-items:center;gap:7px;padding:5px 10px 5px 5px;border-radius:999px;border:1px solid rgba(156,195,228,.22);background:rgba(156,195,228,.05);color:#d6e6f1;font:12px "Segoe UI",Inter,Arial,sans-serif}
.mfav button .mono{width:22px;height:22px;font-size:9px}
.mfav button[aria-pressed=true]{border-color:#9CC3E4;background:rgba(156,195,228,.14)}
.mlist{flex:1;overflow:auto;padding:2px 8px 6px;scrollbar-width:thin;scrollbar-color:#2c3e4f transparent}
.mgrp{padding:12px 10px 6px;font:9.5px Consolas,monospace;letter-spacing:.2em;color:#7d97ab}
.eng{display:flex;align-items:center;gap:11px;padding:8px 10px;border-radius:12px;cursor:pointer;border:1px solid transparent}
.eng:hover,.eng.act{background:rgba(156,195,228,.08)}
.eng.act{border-color:rgba(156,195,228,.28)}
.eng[aria-selected=true]{background:rgba(156,195,228,.12);border-color:rgba(156,195,228,.5)}
.eng[aria-selected=true] .mono{border-color:#F4F6F8;box-shadow:0 0 12px rgba(156,195,228,.45)}
.eng .en{flex:1;min-width:0;display:flex;flex-direction:column;gap:2px}
.eng .en b{font:500 13.5px "Segoe UI",Inter,Arial,sans-serif;color:#F4F6F8}
.eng .en small{font:10px Consolas,monospace;letter-spacing:.04em;color:#8ea6b8;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.eng .pais{font:10px Consolas,monospace;color:#6f889c;white-space:nowrap}
.eng .star{width:26px;height:26px;display:grid;place-items:center;border-radius:8px;color:#5f7789;font-size:14px}
.eng .star:hover{color:#cfe0ec;background:rgba(156,195,228,.1)}
.eng .star.on{color:#F4F6F8}
.mvacio{padding:18px;color:#7891a5;font-size:12px}
.mfoot{padding:10px 18px 12px;border-top:1px solid rgba(156,195,228,.12);font:9.5px Consolas,monospace;letter-spacing:.06em;color:#6c8597;line-height:1.6}
.lens.elegido .lens-body:after{content:"";position:absolute;inset:0;border-radius:inherit;pointer-events:none;background:linear-gradient(100deg,transparent 30%,rgba(156,195,228,.22) 45%,rgba(244,246,248,.32) 50%,rgba(184,170,255,.16) 56%,transparent 70%) no-repeat;background-size:220% 100%;animation:corriente .42s ease-out}
@keyframes corriente{from{background-position:120% 0}to{background-position:-30% 0}}
'''
rep("\n.director{position:fixed", "\n" + SEL_CSS + ".director{position:fixed")
rep('<button type="button" id="searcher" aria-expanded="false">PRIVADOS ⌄</button>',
    '<button type="button" id="searcher" aria-expanded="false" aria-haspopup="listbox" title="Elegir buscador"><i class="mono" id="engmono">G</i><span id="engname">Google</span><span aria-hidden="true">⌄</span></button>')

SEL_JS = r'''// Engines: name, monogram, honest note (focus · source of results), country of headquarters, search address. No logos, no scores.
const ENG=[
['PRIVACIDAD','DuckDuckGo','D','Privacidad · Sin historial de búsquedas','EE. UU.','https://duckduckgo.com/?q='],
['PRIVACIDAD','Brave Search','Br','Privacidad · Índice propio','EE. UU.','https://search.brave.com/search?q='],
['PRIVACIDAD','Startpage','S','Privacidad · Resultados de Google','Países Bajos','https://www.startpage.com/do/search?q='],
['PRIVACIDAD','Qwant','Q','Privacidad · Índice propio y de Bing','Francia','https://www.qwant.com/?q='],
['PRIVACIDAD','Mojeek','M','Privacidad · Índice propio','Reino Unido','https://www.mojeek.com/search?q='],
['PRIVACIDAD','Swisscows','Sw','Privacidad · Filtro familiar','Suiza','https://swisscows.com/web?query='],
['PRIVACIDAD','MetaGer','Mg','Privacidad · Metabuscador','Alemania','https://metager.org/meta/meta.ger3?eingabe='],
['PRIVACIDAD','Ecosia','E','Planta árboles · Resultados de Bing y Google','Alemania','https://www.ecosia.org/search?q='],
['PRIVACIDAD','Kagi','K','De pago · Sin anuncios','EE. UU.','https://kagi.com/search?q='],
['GENERALES','Google','G','General · Índice propio','EE. UU.','https://www.google.com/search?q='],
['GENERALES','Bing','B','General · Índice propio','EE. UU.','https://www.bing.com/search?q='],
['GENERALES','Yahoo','Y!','General · Resultados de Bing','EE. UU.','https://search.yahoo.com/search?p='],
['POR PAÍS','Yandex','Ya','Índice propio','Rusia','https://yandex.com/search/?text='],
['POR PAÍS','Baidu','Ba','Índice propio','China','https://www.baidu.com/s?wd='],
['POR PAÍS','Naver','N','Portal · Índice propio','Corea del Sur','https://search.naver.com/search.naver?query='],
['POR PAÍS','Daum','Da','Portal','Corea del Sur','https://search.daum.net/search?q='],
['POR PAÍS','Seznam','Sz','Índice propio','Chequia','https://search.seznam.cz/?q='],
['POR PAÍS','Yahoo Japón','YJ','Resultados de Google','Japón','https://search.yahoo.co.jp/search?p='],
['POR PAÍS','Sogou','So','Portal','China','https://www.sogou.com/web?query='],
['INTELIGENCIA ARTIFICIAL','Perplexity','P','Respuestas con IA · Servicio externo','EE. UU.','https://www.perplexity.ai/search?q='],
['INTELIGENCIA ARTIFICIAL','You.com','Yo','Respuestas con IA · Servicio externo','EE. UU.','https://you.com/search?q='],
['INTELIGENCIA ARTIFICIAL','ChatGPT','C','Respuestas con IA · Servicio externo','EE. UU.','https://chatgpt.com/?q=']];
const engOf=n=>ENG.find(e=>e[1]===n)||ENG[9];
const guarda=(k,v)=>{try{localStorage.setItem(k,JSON.stringify(v))}catch(e){}},lee=(k,d)=>{try{const v=JSON.parse(localStorage.getItem(k));return v==null?d:v}catch(e){return d}};
let selected=lee('zeroBuscador','Google');if(!ENG.some(e=>e[1]===selected))selected='Google';
let favs=lee('zeroFavs',['Google','Brave Search','DuckDuckGo']),recs=lee('zeroRecs',[]),act=-1;
const menu=$('#menu');menu.setAttribute('role','dialog');
menu.innerHTML='<div class="mhead"><span>ELIGE DÓNDE BUSCAR</span><b id="mactual"></b></div><input class="mfind" id="mfind" type="text" spellcheck="false" autocomplete="off" placeholder="Buscar entre los buscadores…" aria-label="Buscar entre los buscadores"><div class="mfav" id="mfav" aria-label="Favoritos y recientes"></div><div class="mlist" id="mlist" role="listbox" aria-label="Buscadores"></div><div class="mfoot">Agrupados por enfoque: no es un ranking de privacidad.<br>↑ ↓ para moverte · Enter para elegir · ☆ para fijar · Esc para cerrar</div>';
function pintaBoton(){const e=engOf(selected);$('#engmono').textContent=e[2];$('#engname').textContent=e[1];$('#mactual').textContent='ACTUAL · '+e[1].toUpperCase()}
function pintaFav(){const f=$('#mfav');f.textContent='';const lista=[...new Set([...favs,...recs])].slice(0,6);for(const n of lista){const e=engOf(n);const b=document.createElement('button');b.type='button';b.setAttribute('aria-pressed',String(n===selected));b.innerHTML='<i class="mono"></i><span></span>';b.firstChild.textContent=e[2];b.lastChild.textContent=e[1];b.onclick=()=>elige(n);f.append(b)}}
function pintaLista(){const q=$('#mfind').value.trim().toLowerCase();const L=$('#mlist');L.textContent='';let g='',n=0;for(const e of ENG){if(q&&!(e[1]+' '+e[3]+' '+e[4]+' '+e[0]).toLowerCase().includes(q))continue;if(e[0]!==g){g=e[0];const h=document.createElement('div');h.className='mgrp';h.textContent=g;L.append(h)}const r=document.createElement('div');r.className='eng';r.setAttribute('role','option');r.dataset.n=e[1];r.setAttribute('aria-selected',String(e[1]===selected));r.innerHTML='<i class="mono"></i><span class="en"><b></b><small></small></span><span class="pais"></span><span class="star" role="button" tabindex="-1"></span>';r.querySelector('.mono').textContent=e[2];r.querySelector('b').textContent=e[1];r.querySelector('small').textContent=e[3];r.querySelector('.pais').textContent=e[4];const st=r.querySelector('.star');const on=favs.includes(e[1]);st.textContent=on?'★':'☆';st.classList.toggle('on',on);st.setAttribute('aria-label',(on?'Quitar de':'Fijar en')+' favoritos: '+e[1]);st.onclick=ev=>{ev.stopPropagation();favs=on?favs.filter(x=>x!==e[1]):[e[1],...favs].slice(0,5);guarda('zeroFavs',favs);pintaFav();pintaLista()};r.onclick=()=>elige(e[1]);r.onmousemove=()=>marca([...L.querySelectorAll('.eng')].indexOf(r));L.append(r);n++}if(!n){const v=document.createElement('div');v.className='mvacio';v.textContent='Ningún buscador coincide con «'+q+'».';L.append(v)}act=-1}
function marca(i){const rs=[...$('#mlist').querySelectorAll('.eng')];if(!rs.length)return;act=(i+rs.length)%rs.length;rs.forEach((r,k)=>r.classList.toggle('act',k===act));rs[act].scrollIntoView({block:'nearest'})}
function abre(on){showMenu=on;menu.classList.toggle('show',on);$('#searcher').setAttribute('aria-expanded',String(on));if(on){$('#mfind').value='';pintaFav();pintaLista();setTimeout(()=>$('#mfind').focus(),30)}}
function elige(n){selected=n;guarda('zeroBuscador',n);recs=[n,...recs.filter(x=>x!==n)].slice(0,3);guarda('zeroRecs',recs);pintaBoton();abre(false);const L=$('#lens');L.classList.remove('elegido');void L.offsetWidth;L.classList.add('elegido');pulse=Math.max(pulse,.8);input.focus()}
$('#mfind').addEventListener('input',pintaLista);
$('#mfind').addEventListener('keydown',e=>{if(e.key==='ArrowDown'){e.preventDefault();marca(act+1)}else if(e.key==='ArrowUp'){e.preventDefault();marca(act-1)}else if(e.key==='Enter'){e.preventDefault();const rs=[...$('#mlist').querySelectorAll('.eng')];const r=rs[act<0?0:act];if(r)elige(r.dataset.n)}});
document.addEventListener('pointerdown',e=>{if(showMenu&&!menu.contains(e.target)&&!$('#searcher').contains(e.target))abre(false)});
pintaBoton();
$('#searcher').onclick=()=>abre(!menu.classList.contains('show'));'''
line("$('#searcher').onclick=", "")
line("for(const [title,items] of Object.entries(menus))", "")
line("const menus={", SEL_JS)
# the demo page says where the query would go
rep("$('#echo').textContent=val?'Tu consulta de ejemplo: «'+val+'» · Buscador elegido: '+selected:'';",
    "$('#echo').textContent=val?'Tu consulta de ejemplo: «'+val+'» · Se buscaría en '+selected+': '+engOf(selected)[5]+encodeURIComponent(val):'';")

# ---- professional chevron: drawn, aligned, rotates when the list is open ----
rep('<span id="engname">Google</span><span aria-hidden="true">⌄</span></button>',
    '<span id="engname">Google</span><svg class="chev" viewBox="0 0 12 12" aria-hidden="true"><path d="M3 4.6 6 7.6 9 4.6" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg></button>')
rep("#searcher .mono{width:24px;height:24px;font-size:10px}",
    "#searcher .mono{width:24px;height:24px;font-size:10px}#searcher #engname{flex:1;text-align:left}#searcher .chev{width:12px;height:12px;margin-left:2px;color:#9CC3E4;opacity:.85;transition:transform .25s cubic-bezier(.2,.85,.2,1),opacity .2s}#searcher:hover .chev{opacity:1}#searcher[aria-expanded=true] .chev{transform:rotate(180deg)}#searcher[aria-expanded=true]{border-color:rgba(156,195,228,.5);background:rgba(156,195,228,.12)}")
