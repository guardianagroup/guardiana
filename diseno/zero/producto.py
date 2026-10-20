# In the program (opened from Órbita with ?orbita=1): the real browser does the browsing.
# The engine list matches ZERO's own (crates/zero/src/direccion.rs): no Kagi; YouTube, Ask, AOL, 360 Search and Cốc Cốc added.
rep("['PRIVACIDAD','Kagi','K','De pago · Sin anuncios','EE. UU.','https://kagi.com/search?q='],\n", "")
rep("['GENERALES','Yahoo','Y!','General · Resultados de Bing','EE. UU.','https://search.yahoo.com/search?p='],",
    "['GENERALES','Yahoo','Y!','General · Resultados de Bing','EE. UU.','https://search.yahoo.com/search?p='],\n"
    "['GENERALES','YouTube','YT','Vídeos','EE. UU.','https://www.youtube.com/results?search_query='],\n"
    "['GENERALES','Ask','A','General','EE. UU.','https://www.ask.com/web?q='],\n"
    "['GENERALES','AOL','AO','General · Resultados de Bing','EE. UU.','https://search.aol.com/aol/search?q='],")
rep("['POR PAÍS','Sogou','So','Portal','China','https://www.sogou.com/web?query='],",
    "['POR PAÍS','Sogou','So','Portal','China','https://www.sogou.com/web?query='],\n"
    "['POR PAÍS','360 Search','36','Portal','China','https://www.so.com/s?q='],\n"
    "['POR PAÍS','Cốc Cốc','CC','Portal','Vietnam','https://coccoc.com/search?query='],")

PROD_JS = r'''
// ---------- In the program (opened from Órbita): ZERO's own window browses for real ----------
// The same messages the old start section sent: the engine as a setting, then navigate and open the shield's panel.
const PROD=/[?&]orbita=1/.test(location.search);
const HOSTZ=(()=>{try{return (window.chrome&&window.chrome.webview)||(parent!==window&&parent.chrome&&parent.chrome.webview)||null}catch(e){return null}})();
const IDZ={'DuckDuckGo':'duckduckgo','Brave Search':'brave','Startpage':'startpage','Qwant':'qwant','Mojeek':'mojeek','Swisscows':'swisscows','MetaGer':'metager','Ecosia':'ecosia','Google':'google','Bing':'bing','Yahoo':'yahoo','YouTube':'youtube','Ask':'ask','AOL':'aol','Yandex':'yandex','Baidu':'baidu','Naver':'naver','Daum':'daum','Seznam':'seznam','Yahoo Japón':'yahoojp','Sogou':'sogou','360 Search':'so360','Cốc Cốc':'coccoc','Perplexity':'perplexity','You.com':'you','ChatGPT':'chatgpt'};
function avisaMotor(){if(HOSTZ&&IDZ[selected])try{HOSTZ.postMessage({tipo:'ajuste',clave:'buscador',valor:IDZ[selected]})}catch(e){}}
{const pb=pintaBoton;pintaBoton=function(){pb();avisaMotor()}}
if(PROD){
  {const ow=openWeb;openWeb=function(){const val=input.value.trim();if(!val){input.focus();return}
    if(HOSTZ){try{HOSTZ.postMessage({tipo:'navegar',texto:val});HOSTZ.postMessage({tipo:'panel',vista:'escudo'})}catch(e){}return}
    $('.hint').textContent='Para navegar, abre la ventana de ZERO: allí la web aparece en el centro y lo que se corta, a la derecha.';$('.hint').style.opacity='1'}}
  const aOrbita=()=>{try{const v=parent!==window&&parent.document.getElementById('volver');if(v){v.click();return}}catch(e){}history.back()};
  $('#back').onclick=aOrbita;$('#back').setAttribute('aria-label','Volver a Órbita');
  document.addEventListener('keydown',e=>{if(e.key==='Escape'&&!opened&&!showMenu&&!$('#upanel').classList.contains('show')&&!stage.classList.contains('intro'))aOrbita()});
  setTimeout(()=>{const d=$('#director');if(d)d.remove()},0);document.querySelectorAll('.edge').forEach(e=>e.remove());$('.eyebrow').textContent='';
}
pintaBoton();
'''
rep("document.addEventListener('keydown',e=>{if(e.key==='Escape')panelUltra(false)});",
    "document.addEventListener('keydown',e=>{if(e.key==='Escape')panelUltra(false)});" + PROD_JS)
# in the program the entrance plays once per session (then ZERO opens straight onto the living bar)
rep("if(q&&+q[1]<ENTRADAS.length)i=+q[1];play(i)}",
    "if(q&&+q[1]<ENTRADAS.length)i=+q[1];if(/[?&]orbita=1/.test(location.search)){i=0;let vista=false;try{vista=sessionStorage.getItem('zeroIntro')==='1';sessionStorage.setItem('zeroIntro','1')}catch(e){}if(vista){cineMode=0;stage.classList.add('vivo');endCine()}else play(i)}else play(i)}")
