// ---------- Cinematic entrance: sequences on the same organism ----------
// In the product it plays only the first time per session and never with reduced motion; here it plays on every load.
const META={Bm:1,Bk:0,cy:450};
const ENTRADAS=[{id:'mpunto',n:'Metamorfosis · punto',d:7.5,rest:META},{id:'mabajo',n:'Metamorfosis · desde abajo',d:7.5,rest:META},{id:'mred',n:'Metamorfosis · conexión',d:7.5,rest:META}];
const REST={As:1,Sp:0,Ap:1,Ir:0,S0:155,cx:720,cy:250,Hz:0,Ry:.35,Ga:1,Hb:0,fl:1,Mm:1,Mr:1,Cl:-2000,Df:1,Bm:0,Bk:0,Av:0,Sw:0,Sa:0,Sq:1,Cw:-1,Il:.4,An:0,Af:0,ex:720,ey:450,Re:200,Er:0,Eo:0,Ds:0,Du:0,Pv:0,Fa:-1};
let conRed=false; // final: the network also stays above the bar
const cine=Object.assign({},REST);
let cineMode=0,cineStart=-1,cineFixed=null;
const ss=(a,b,x)=>{x=Math.max(0,Math.min(1,(x-a)/(b-a)));return x*x*(3-2*x)};
const ez=(a,b,x)=>{x=Math.max(0,Math.min(1,(x-a)/(b-a)));return x<.5?4*x*x*x:1-Math.pow(-2*x+2,3)/2};
const beat=(x,c)=>Math.exp(-((x-c)**2)*160);
const heart=x=>{const ph=x%5;return beat(ph,.2)+.6*beat(ph,.55)};
function direct(T){
  const m=ENTRADAS[cineMode].id,c=Object.assign({},REST),v={bo:0,bx:0,by:450,sy:1,lo:1,lc:1,ui:1,fl:1,lb:0,fo:0,fy:50};
  if(m[0]==='m'){ // METAMORPHOSIS: a neural network arrives, thinks, inhales, and turns itself into the search bar
    const T0=T;if(m==='mplaneta')T=T0-2.3;if(m==='mcaida')T=T0-2.75;if(m==='marena')T=T0-1;
    if(m==='mpunto'){c.Ga=ss(.25,.42,T);c.As=ss(.9,2.6,T);c.S0=55+75*ez(.9,2.7,T);c.cy=450;c.Hb=1.2*beat(T,.35)+.8*beat(T,.7);c.Sq=1;}
    else if(m==='mabajo'){c.Hz=T<2.9?1:0;c.S0=130;c.cy=790-390*ez(.9,2.9,T)+50*ez(2.9,3.3,T);c.Ga=ss(.5,1.1,T);v.bo=ss(.1,.4,T)*(1-ss(3,3.4,T));v.bx=ez(.1,1.1,T);v.by=565;c.Hb=beat(T,.3);}
    else if(m==='mplaneta'){ // the real Earth, alive, dissolves into living cosmic dust that orders itself into the network
      c.Av=2;c.ex=720;c.ey=450;c.Re=200+25*ez(0,2.2,T0);c.Eo=ss(0,.9,T0);c.Er=-.45+Math.min(T0,2.2)*.13;c.Ds=ez(1.9,2.9,T0);c.Pv=ss(1.85,2.3,T0);c.Du=ez(2.1,3.4,T0);c.As=ss(3,4.7,T0);c.S0=130;c.cy=450;c.Ga=1;}
    else if(m==='mcaida'){ // ChatGPT's mix: the Earth turns to cosmic sand and falls; then part of it stops in mid-air and the rest rises, defying gravity, into the network
      c.Av=2;c.ex=720;c.ey=430;c.Re=205+20*ez(0,1.9,T0);c.Eo=ss(0,.9,T0);c.Er=-.45+Math.min(T0,2)*.13;c.Ds=ez(1.9,3,T0);c.Pv=ss(1.85,2.3,T0);c.Du=.35*ez(2,3,T0);c.Fa=T0>2.3?T0-2.3:-1;c.As=ss(3.6,5.3,T0);c.S0=130;c.cy=450;c.Ga=1;}
    else if(m==='marena'){ // the client's idea: the Earth turns into cosmic sand that falls and, as it lands, forms the bar
      c.Av=3;c.ex=720;c.ey=235;c.Re=160+8*ez(0,2,T0);c.Eo=ss(0,.9,T0);c.Er=-.45+Math.min(T0,2.2)*.13;c.Ds=Math.max(0,Math.min(1,(T0-2)/2.65));c.Pv=ss(1.95,2.05,T0);c.Fa=T0>2?T0-2:-1;c.As=0;c.cy=235+215*ss(3.5,5.5,T0);c.Ga=1;}
    else{c.Av=1;c.Ga=ss(0,.9,T);c.As=ez(.3,2.6,T);c.S0=130;c.cy=450;}
    c.Ir=2.2*ss(1.6,2.5,T)*(1-ss(3.3,4.3,T));c.Hb=Math.max(c.Hb,1.4*beat(T,2.75)+beat(T,3));c.Ry=.35+.75*ss(1.4,2.6,T)*(1-ss(3.4,4.6,T));c.Il=1;
    c.S0*=1-.18*ss(3,3.3,T)*(1-ss(3.35,3.7,T));
    {const x=Math.max(0,Math.min(1,(T-3.35)/1.45));c.Bm=1-Math.pow(1-x,3.6);}v.fo=.16*Math.exp(-((T-3.45)**2)*80);
    if(T>3.3){c.Sq=4.5;c.Sw=1150*ez(3.38,4.3,T);c.Sa=.5*(1-ss(3.5,4.3,T));}
    v.lo=ss(4.3,4.9,T);v.lbz=16*(1-ez(4.3,5.2,T));v.rx=1-ez(5.0,5.6,T);v.ro=.45+.55*ss(5,5.2,T)*(1-ss(5.6,6.2,T));c.Il=T<5.6?1:T<5.85?.15:.15+.85*ss(5.85,6.0,T)*(1-ss(6.4,6.9,T))+.4*ss(6.4,6.9,T);c.Cw=T>5.85&&T<6.5?ez(5.85,6.45,T):-1;if(T>5.55)c.Ir=0;v.mk=ss(5.95,6.3,T)*(1-ss(6.6,6.95,T));v.lc=ss(6.85,7.2,T);v.sg=1.6*ss(4.4,5,T)*(1-.45*ss(5.4,6,T));
    v.fl=0;v.ui=ss(6.9,7.4,T);v.lb=110*(1-ez(6.2,7.4,T));
    if(m==='marena'){c.An=0;c.Af=0;c.Sa=0;v.lb*=.55;}
    if(conRed){c.cy=T>4.7?250:c.cy;c.Bk=ez(5.2,6.3,T);}
  }else if(m==='eclosion'){ // a point of light beats twice, the organism spirals out of it, rises, and the bar is born from a seam of light
    c.Ga=ss(0,.5,T);c.As=ss(.7,3,T);c.cy=450-200*ez(2.9,3.9,T);c.Ir=1.6*ss(2.3,2.9,T)*(1-ss(3.9,4.8,T));c.Ry=.35+1.2*ss(1.6,2.6,T)*(1-ss(3.4,4.8,T));c.Hb=T<1.2?beat(T,.3)+.7*beat(T,.62):0;
    v.bo=ss(3.6,3.8,T)*(1-ss(4.6,5,T));v.bx=ez(3.6,4.2,T);v.sy=.03+.97*ez(4.1,4.7,T);v.lo=ss(4.05,4.2,T);v.lc=ss(4.5,5,T);v.fl=ss(4.5,5.2,T);v.ui=ss(5,5.8,T);v.lb=110*(1-ez(4.7,5.8,T));v.fo=.45*Math.exp(-((T-3.7)**2)*60);
  }else if(m==='iris'){ // the camera starts inside the organism and pulls back; the iris opens like a diaphragm and lets out the bar
    c.Ga=ss(0,.8,T);c.S0=155+2700*Math.pow(1-ez(0,2.6,T),2);c.Ap=ez(2.5,3.3,T);c.cy=450-200*ez(3.1,4.1,T);c.Ir=1.4*ss(2.8,3.4,T)*(1-ss(4,4.8,T));c.Ry=.35+1.3*(1-ss(2.6,4.8,T));c.Hb=T<2.4?.6+.4*Math.sin(T*9):0;
    v.fo=.75*Math.exp(-((T-2.6)**2)*40);v.bo=ss(3.8,4,T)*(1-ss(4.8,5.2,T));v.bx=ez(3.8,4.4,T);v.sy=.03+.97*ez(4.3,4.9,T);v.lo=ss(4.25,4.4,T);v.lc=ss(4.7,5.1,T);v.fl=ss(4.6,5.3,T);v.ui=ss(5.1,5.9,T);v.lb=110*(1-ez(4.8,5.9,T));
  }else if(m==='horizonte'){ // a horizon of light, the organism rises from under it like a sunrise, and the horizon lifts into the bar
    c.Hz=T<4.4?1:0;c.cy=820-570*ez(1,3.8,T);c.Ga=ss(.6,1.2,T);c.Ir=1.2*ss(2.6,3.3,T)*(1-ss(4,4.8,T));c.Ry=.35+1.2*(1-ss(3,5,T));
    v.bo=ss(.1,.4,T)*(1-ss(4.4,4.9,T));v.bx=ez(.1,1.1,T);v.by=565-115*ez(3.6,4.2,T);v.sy=.03+.97*ez(4,4.6,T);v.lo=ss(3.95,4.1,T);v.lc=ss(4.5,5,T);v.fl=Math.max(.55*ss(.4,1.4,T),ss(4.2,5,T));v.ui=ss(5,6,T);v.lb=110*(1-ez(4.8,6,T));v.fo=.4*Math.exp(-((T-4.05)**2)*50);
  }else if(m==='espejo'){ // after ChatGPT's «El otro lado del espejo»: first only a reflection of something that is not there; it emerges, and the bar is born from its own reflection
    c.cy=330-80*ez(3.9,4.9,T);c.S0=155+20*ss(2.4,3.2,T)*(1-ss(3.9,4.9,T));c.Mm=T<1.6?0:1;c.Cl=T<2.9?565-560*ez(1.65,2.8,T):-2000;c.Mr=1.8*ss(.7,1.6,T)*(1-.45*ss(2.8,4,T));c.Ir=1.3*ss(2.2,2.9,T)*(1-ss(4.2,5.2,T));c.Ry=.35+1.1*ss(1.8,2.8,T)*(1-ss(4.5,6,T));
    v.bo=ss(.05,.35,T)*(1-ss(4.6,5.2,T));v.bx=ez(.05,.9,T);v.by=565;v.fl=Math.max(.6*ss(.3,1,T),ss(5.2,6,T));v.rf=2.6*ss(3.5,4.3,T)*(1-.6*ss(4.8,5.6,T));
    v.lo=ss(4.5,4.65,T);v.sy=.03+.97*ez(4.5,5.3,T);v.lc=ss(5.2,5.8,T);v.ui=ss(5.6,6.3,T);v.lb=110*(1-ez(5.3,6.4,T));v.fo=.3*Math.exp(-((T-4.6)**2)*50);
  }else{ // division: the organism splits in two like a cell, the bar is born in the gap, and the halves rejoin above it
    c.Ga=ss(0,.5,T);c.As=ss(.1,1.5,T);c.S0=100+55*ez(2.9,4.3,T);c.cy=450-200*ez(2.9,4.3,T);c.Sp=560*ez(1.5,2.5,T)*(1-ez(3,4.2,T));c.Ir=1.4*ss(1.2,1.8,T)*(1-ss(3.6,4.4,T));c.Ry=.35+1.1*(1-ss(3,5,T));
    v.bo=ss(1.8,2,T)*(1-ss(3.2,3.7,T));v.bx=ez(1.8,2.5,T);v.sy=.03+.97*ez(2.35,3,T);v.lo=ss(2.3,2.45,T);v.lc=ss(2.9,3.4,T);v.fo=.5*Math.exp(-((T-1.9)**2)*60);v.fl=ss(3.6,4.4,T);v.ui=ss(4.4,5.4,T);v.lb=110*(1-ez(4.2,5.4,T));
  }
  if(v.rf==null)v.rf=v.fl;if(v.lbz==null)v.lbz=0;if(v.sg==null)v.sg=0;if(v.mk==null)v.mk=0;if(v.rx==null)v.rx=1;if(v.ro==null)v.ro=.45;c.fl=v.fl;Object.assign(cine,c);
  const st=stage.style,set=(k,x)=>st.setProperty('--'+k,x);
  for(const k of ['bo','bx','sy','lo','lc','ui','fl','fo','rf'])set(k,v[k].toFixed(4));
  set('by',v.by.toFixed(1)+'px');set('lbz',v.lbz.toFixed(1)+'px');set('sg',v.sg.toFixed(3));set('mk',v.mk.toFixed(3));set('rx',v.rx.toFixed(3));set('ro',v.ro.toFixed(3));set('lb',v.lb.toFixed(1)+'px');set('fy',v.fy+'%');
}
function restOf(){const r=ENTRADAS[cineMode].rest;return r?Object.assign({},r,conRed?{Bk:1,cy:250}:{}):{}}
function endCine(){cineStart=-1;cineFixed=null;Object.assign(cine,REST,restOf());stage.classList.remove('intro');for(const k of ['bo','bx','by','sy','lo','lc','ui','fl','lb','fo','fy','rf','lbz','sg','mk','rx','ro'])stage.style.removeProperty('--'+k);if(!opened)input.focus({preventScroll:true})}
function cineFrame(ms){
  if(cineStart<0){cine.Hb=reduced?0:.55*heart(elapsed);if(cine.Bm>.5)stage.style.setProperty('--sg',(.8+.55*cine.Hb).toFixed(3));
    // the iris blinks every 17 s (after Grok): quick close, slower reopening
    const x=(elapsed%17-16)/.75;cine.Ap=reduced||x<0||x>1?1:1-.85*Math.pow(Math.sin(Math.PI*Math.pow(x,.7)),1.4);return}
  const T=cineFixed!=null?cineFixed:(ms-cineStart)/1000;
  if(T>=ENTRADAS[cineMode].d){endCine();return}
  direct(T);
}

// ---------- Sound: synthesized in the page (no files). Browsers only allow it after a click, so it sounds when ZERO is opened by a click.
let actx=null,master=null,sonido=true,gesto=false;
addEventListener('pointerdown',()=>{gesto=true},true);addEventListener('keydown',()=>{gesto=true},true);
function ac(){if(!gesto)return null;if(!actx){try{actx=new (window.AudioContext||window.webkitAudioContext)();const k=actx.createDynamicsCompressor();master=actx.createGain();master.gain.value=.55;master.connect(k).connect(actx.destination)}catch(e){actx=null}}if(actx&&actx.state==='suspended')actx.resume();return actx}
function tone(t,f,dur,g,type,f2){const o=actx.createOscillator(),v=actx.createGain();o.type=type||'sine';o.frequency.setValueAtTime(f,t);if(f2)o.frequency.exponentialRampToValueAtTime(f2,t+dur);v.gain.setValueAtTime(0,t);v.gain.linearRampToValueAtTime(g,t+.012);v.gain.exponentialRampToValueAtTime(.0001,t+dur);o.connect(v).connect(master);o.start(t);o.stop(t+dur+.05)}
function swell(t,dur,g,f1,f2,q){const n=Math.floor(actx.sampleRate*dur),b=actx.createBuffer(1,n,actx.sampleRate),d=b.getChannelData(0);for(let i=0;i<n;i++)d[i]=Math.random()*2-1;const s=actx.createBufferSource();s.buffer=b;const f=actx.createBiquadFilter();f.type='bandpass';f.Q.value=q;f.frequency.setValueAtTime(f1,t);f.frequency.exponentialRampToValueAtTime(f2,t+dur);const v=actx.createGain();v.gain.setValueAtTime(.0001,t);v.gain.exponentialRampToValueAtTime(g,t+dur*.75);v.gain.exponentialRampToValueAtTime(.0001,t+dur);s.connect(f).connect(v).connect(master);s.start(t)}
const latido=t=>{tone(t,58,.42,.9,'sine',36);tone(t,116,.14,.18)};
function banda(m){if(!sonido||!ac())return;const t=actx.currentTime+.03;const o=m==='mcaida'?2.75:m==='mplaneta'?2.3:m==='marena'?1:0;
  if(m==='mplaneta'||m==='marena'||m==='mcaida'){swell(t+.05,2.2,.16,55,180,1.1);swell(m==='marena'?t+2:t+1.85,m==='marena'?3.9:1.5,.14,1600,7500,m==='marena'?1.2:4);}
  if(m==='mpunto'){latido(t+.35);latido(t+.7);}else{latido(t+2.75+o);latido(t+3+o);}
  if(m!=='marena'){swell(t+3.36+o,1.3,.34,220,1800,.7);tone(t+3.38+o,48,1.1,.6,'sine',30);}
  tone(t+6+o,1568,2.4,.10);tone(t+6+o,2349,1.8,.05);tone(t+6.02+o,3136,1.2,.03);}
const dir=$('#director');
function paintDirector(){dir.querySelectorAll('button[data-i]').forEach(b=>b.setAttribute('aria-pressed',String(+b.dataset.i===cineMode)))}
function play(i){cineMode=i;try{localStorage.setItem('zeroEntrada2',String(i))}catch(e){}paintDirector();stage.classList.toggle('vivo',!!ENTRADAS[i].rest);if(reduced){endCine();return}if(opened)orbit();cineStart=performance.now();cineFixed=null;banda(ENTRADAS[i].id);stage.classList.add('intro');input.blur();direct(0)}
ENTRADAS.forEach((e,i)=>{const b=document.createElement('button');b.type='button';b.dataset.i=i;b.textContent=e.n;b.onclick=()=>play(i);dir.append(b)});
{const b=document.createElement('button');b.type='button';b.textContent='↻ Repetir';b.onclick=()=>play(cineMode);dir.append(b)}
{const b=document.createElement('button');b.type='button';b.id='fin';const pinta=()=>{b.textContent=conRed?'Final: barra + red encima':'Final: solo barra viva';b.setAttribute('aria-pressed',String(conRed))};pinta();b.onclick=()=>{conRed=!conRed;pinta();if(ENTRADAS[cineMode].rest)play(cineMode)};dir.append(b)}
{const b=document.createElement('button');b.type='button';const pinta=()=>{b.textContent=sonido?'♪ Sonido: sí':'♪ Sonido: no';b.setAttribute('aria-pressed',String(sonido))};pinta();b.onclick=()=>{sonido=!sonido;pinta();if(sonido)play(cineMode)};dir.append(b)}
// any key, click or wheel skips the entrance (the director's own buttons excepted)
const skip=e=>{if(cineStart<0||cineFixed!=null)return;if(e.target&&e.target.closest&&e.target.closest('#director'))return;if(e.type!=='wheel'){e.preventDefault();e.stopPropagation()}endCine()};
addEventListener('keydown',skip,true);addEventListener('pointerdown',skip,true);addEventListener('wheel',skip,{capture:true,passive:true});
window.zeroCine={play,red(x){conRed=!!x},at(i,t){cineMode=i;paintDirector();stage.classList.toggle('vivo',!!ENTRADAS[i].rest);stage.classList.add('intro');cineStart=0;cineFixed=t;direct(t)},end:endCine};
{let i=0;const q=/entrada=(\d)/.exec(location.search);try{const s=parseInt(localStorage.getItem('zeroEntrada2'),10);if(s>=0&&s<ENTRADAS.length)i=s}catch(e){}if(q&&+q[1]<ENTRADAS.length)i=+q[1];play(i)}
