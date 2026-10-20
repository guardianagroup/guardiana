import sys, re
src, dst = sys.argv[1], sys.argv[2]
s = open(src, encoding='utf-8').read()

def rep(old, new, count=1):
    global s
    n = s.count(old)
    if n != count:
        raise SystemExit(f'expected {count} match(es), found {n}: {old[:70]!r}')
    s = s.replace(old, new)

def line(prefix, new):
    global s
    L = s.split('\n')
    idx = [i for i, l in enumerate(L) if l.startswith(prefix)]
    if len(idx) != 1:
        raise SystemExit(f'line {prefix!r}: {len(idx)}')
    L[idx[0]] = new
    s = '\n'.join(L)

rep('<title>GUARDIANA ZERO · NÁCAR — Prototipo de arte interactivo</title>', '<title>ZERO · NÁCAR Cine</title>')

# ---------- CSS ----------
CSS = r'''/* Cinematic entrance: a seam of light, letterbox, flash, and the glass bar born from the light. */
.beam{position:absolute;z-index:24;left:0;width:1440px;top:var(--by,450px);height:2px;margin-top:-1px;pointer-events:none;opacity:var(--bo,0);transform:scaleX(var(--bx,0));background:linear-gradient(90deg,transparent,rgba(156,195,228,.55) 24%,#F4F6F8 50%,rgba(156,195,228,.55) 76%,transparent);box-shadow:0 0 16px 2px rgba(156,195,228,.6),0 0 70px 8px rgba(156,195,228,.16)}
.lb{position:absolute;left:0;right:0;z-index:60;background:#000;height:var(--lb,0px);pointer-events:none}.lb.t{top:0}.lb.b{bottom:0}
.flash{position:absolute;inset:0;z-index:30;pointer-events:none;background:radial-gradient(circle at 50% var(--fy,50%),rgba(244,246,248,.95),rgba(156,195,228,.28) 26%,transparent 58%);opacity:var(--fo,0);mix-blend-mode:screen}
.skip{position:absolute;z-index:61;right:46px;bottom:44px;font:10px Consolas,monospace;letter-spacing:.2em;color:#5f778b;opacity:0;transition:opacity .5s;pointer-events:none}
#stage.intro .skip{opacity:1}
#stage.intro .lens{transition:none;transform:translate(-50%,-50%) scale(1,var(--sy,1));opacity:var(--lo,1);filter:blur(var(--lbz,0px))}
#stage.vivo .lens{box-shadow:0 18px 46px #000c,0 3px 9px #000,inset 0 2px 2px rgba(255,255,255,.62),inset 0 -4px 8px #000a,-1px 0 0 rgba(220,233,242,.28),2px 0 0 rgba(57,144,245,.35),-16px 0 34px -8px rgb(156 195 228 / calc(.75*var(--sg,0))),16px 0 34px -8px rgb(156 195 228 / calc(.75*var(--sg,0))),-4px 0 10px -2px rgb(244 246 248 / calc(.7*var(--sg,0))),4px 0 10px -2px rgb(244 246 248 / calc(.7*var(--sg,0)))}
#stage.intro .lens-body>*{opacity:var(--lc,1)}
#stage.intro .mast,#stage.intro .edge,#stage.intro .hint{opacity:var(--ui,1);transition:none}
#stage.intro .floor{opacity:var(--fl,1);transition:none}
#stage.intro .lens-reflect{opacity:calc(var(--rf,1) * .34);transition:none}
.caption{display:none}#stage.open .mast{opacity:0}
#stage.vivo .lens{background:linear-gradient(165deg,rgba(244,246,248,.30),rgba(156,195,228,.07) 9%,rgba(14,23,34,.42) 50%,rgba(141,192,228,.14) 92%,rgba(255,255,255,.32))}#stage.vivo .lens-body{background:linear-gradient(180deg,rgba(59,85,106,.42),rgba(10,16,24,.74) 38%,rgba(6,10,16,.82));box-shadow:inset 0 1px 0 rgba(244,246,248,.55),inset 0 -8px 12px #000c}#stage.vivo .lens-body::after{content:"";position:absolute;inset:0;pointer-events:none;border-radius:inherit;opacity:var(--ro,.45);background:radial-gradient(90px 2px at calc(70px + var(--rx,1) * 35px) 12%,rgba(244,246,248,.75),transparent 72%),radial-gradient(90px 2px at calc(100% - 70px - var(--rx,1) * 35px) 88%,rgba(156,195,228,.7),transparent 72%)}
.marca{position:absolute;z-index:26;left:720px;top:450px;transform:translate(-50%,-50%);font:300 30px/1 "Segoe UI",Inter,Arial,sans-serif;letter-spacing:.62em;padding-left:.62em;color:#F4F6F8;text-shadow:0 0 18px rgba(156,195,228,.9),0 0 46px rgba(156,195,228,.45);opacity:var(--mk,0);pointer-events:none;filter:blur(calc((1 - var(--mk,0)) * 6px))}
.director{position:fixed;z-index:100;left:16px;bottom:16px;display:flex;flex-wrap:wrap;gap:6px;align-items:center;max-width:calc(100vw - 32px);padding:6px;border-radius:14px;background:rgba(6,8,12,.74);border:1px solid rgba(156,195,228,.22);backdrop-filter:blur(10px);font:12px "Segoe UI",Inter,Arial,sans-serif;color:#cfe1ee}
.director span{font:9.5px Consolas,monospace;letter-spacing:.18em;color:#7f97aa;padding:0 6px}
.director button{border:1px solid #2b4152;background:#101a25;color:#cfe1ee;border-radius:9px;padding:7px 11px}
.director button[aria-pressed=true]{border-color:#9cc3e4;color:#fff;background:#18283a}
'''
rep('@media(prefers-reduced-motion:reduce)', CSS + '@media(prefers-reduced-motion:reduce)')

# ---------- HTML ----------
rep('<div class="grain"></div><div class="floor"></div>', '<div class="grain"></div><div class="floor"></div><div class="beam"></div><div class="flash"></div>')
rep('</main></div>', '<div class="marca" aria-hidden="true">ZERO</div><div class="lb t"></div><div class="lb b"></div><div class="skip">CLIC O CUALQUIER TECLA PARA SALTAR</div>\n</main></div>\n<nav class="director" id="director" aria-label="Entradas de cine"><span>ENTRADA</span></nav>')

# mic error: no browser dialog
rep("alert('Micrófono no disponible: '+(err?.message||'Permiso denegado'))", "$('#mic').title='Micrófono no disponible: '+(err?.message||'permiso denegado')")

# ---------- shaders ----------
VERTEX = r'''const vertex=`precision highp float;attribute vec3 aP;attribute vec3 aE;attribute float aS;uniform float T,M,P,V,R,Mirror,Pixel,Still,As,Sp,Ap,Hb,Ir,S0,Ga,Fl,Mm,Mr,Df,Bm,Bk,Av,Ln,Cw,Il,An,Af;uniform vec2 Eye,C0;uniform vec4 Rb;varying float vG,vRed,vA,vBand,vY,vW,vH,vMir;vec4 rrect(float s,vec4 R){float r=min(R.w*.66,R.z);float a=R.z-r,b=2.*(R.w-r),q=1.5708*r;float L=4.*a+2.*b+4.*q;float d=s*L;vec2 c;float th;vec2 n;
if(d<a)return vec4(R.x+d,R.y-R.w,0.,-1.);d-=a;
if(d<q){th=d/r;c=vec2(R.x+a,R.y-R.w+r);n=vec2(sin(th),-cos(th));return vec4(c+n*r,n);}d-=q;
if(d<b)return vec4(R.x+R.z,R.y-R.w+r+d,1.,0.);d-=b;
if(d<q){th=d/r;c=vec2(R.x+a,R.y+R.w-r);n=vec2(cos(th),sin(th));return vec4(c+n*r,n);}d-=q;
if(d<2.*a)return vec4(R.x+a-d,R.y+R.w,0.,1.);d-=2.*a;
if(d<q){th=d/r;c=vec2(R.x-a,R.y+R.w-r);n=vec2(-sin(th),cos(th));return vec4(c+n*r,n);}d-=q;
if(d<b)return vec4(R.x-R.z,R.y+R.w-r-d,-1.,0.);d-=b;
if(d<q){th=d/r;c=vec2(R.x-a,R.y-R.w+r);n=vec2(-cos(th),-sin(th));return vec4(c+n*r,n);}d-=q;
return vec4(R.x-a+d,R.y-R.w,0.,-1.);}
void main(){vec3 q=aP;float t=T*(1.-Still);
float k=clamp((As+Ln*.32-aE.x*.5)/.5,0.,1.);k=k*k*(3.-2.*k);float sw=(1.-k)*2.8;float cs=cos(sw),sn=sin(sw);vec3 qsp=vec3(q.x*cs+q.z*sn,q.y,-q.x*sn+q.z*cs)*mix(.015,1.,pow(k,.75));vec3 qsc=mix(vec3((fract(aE.x*91.7)-.5)*9.,(fract(aE.y*57.3)-.5)*5.6,(fract(aE.x*13.1+aE.y*7.)-.5)*2.),aP,k);q=mix(qsp,qsc,Av);
float an=(1.-Ln)*step(fract(aE.x*7.13),.014);vec3 dir=normalize(vec3(-.94,.22,.3));float u0=fract(aE.y*3.37);vec3 qe=dir*(1.02+u0*1.7*An)+vec3(sin(u0*25.+T*3.)*.025,cos(u0*19.+T*2.)*.025,0.)*An+vec3(0.,-Af*Af*.6*u0,0.);q=mix(q,qe,an*smoothstep(0.,.2,An));float iris=step(.79,aE.z)*step(.3,aP.z)*(1.-step(1.,length(aP.xy)));q.xy*=mix(1.,Ap,iris);
float breath=1.+.027*sin(T*.981)*(1.-Still)+.006*sin(T*1.77);q.xy*=breath;float wave0=sin(q.y*8.+q.x*9.+t*1.45+aE.x*8.)*.017*(1.-Still);q.x+=wave0*q.z;q.z+=.026*sin(q.y*14.+t*1.1+aE.y*9.)*(1.-Still);float ry=t*.055+Eye.x*.16;float rx=Eye.y*.10;float c=cos(ry),s=sin(ry),cx=cos(rx),sx=sin(rx);q=vec3(q.x*c+q.z*s,q.y,-q.x*s+q.z*c);q=vec3(q.x,q.y*cx-q.z*sx,q.y*sx+q.z*cx);float blur=abs(q.z-.62)*Df*(1.-M*.6);float wobble=1.+.05*V+.025*P+.035*Hb;q*=wobble;
vec2 center=mix(C0,vec2(1290.,185.),M);float scale=mix(S0,53.,M);float pers=2.4/(2.4+q.z*.3);vec2 pos=center+vec2(q.x,-q.y)*scale*pers;pos.x+=sign(q.x)*Sp;float grp=step(aE.y,.55);float kb=clamp((Bm-fract(aE.x*3.7)*.35)/.65,0.,1.);kb=kb*kb*(3.-2.*kb);float bb=kb*(1.-Bk*(1.-grp))*(1.-M);float sr=fract(aE.x*7.31+aE.y*3.17+T*.008*(1.-Still));if(fract(aE.x*13.7)<.3)sr=fract((fract(aE.x*29.3)<.5?.75:.25)+(fract(aE.y*41.)-.5)*.14+T*.002);vec4 rr=rrect(sr,Rb);vec2 rim=rr.xy+rr.zw*(3.+8.*fract(aE.y*17.3)+2.*sin(T*1.3+aE.x*20.));float inr=max(step(fract(aE.x*5.3),.22),step(.95,aE.z)*step(aE.y,.5));vec2 ins=vec2(Rb.x+(fract(aE.x*17.1+T*.006*(1.-Still))-.5)*2.*Rb.z*.93,Rb.y+(fract(aE.y*23.7)-.5)*Rb.w*1.5+sin(T*.9+aE.x*40.)*2.5);rim=mix(rim,ins,inr);float capB=smoothstep(.7,1.,abs(rim.x-Rb.x)/Rb.z)*(1.-inr);float cur=exp(-pow((rim.x-(Rb.x-Rb.z+Cw*2.*Rb.z))/46.,2.))*step(-.5,Cw);float rw=mix(pow(max(0.,sin(sr*18.85-T*2.4)),9.),(pow(max(0.,sin((rim.x-Rb.x)*.02-T*3.1+aE.y*2.)),14.)*1.2+P*1.4)*Il+cur*3.,inr);pos=mix(pos,rim,bb);if(Mirror>.5)pos.y=1130.-pos.y;vY=pos.y;vMir=Mirror;gl_Position=vec4(pos.x/720.-1.,1.-pos.y/450.,0.,1.);vBand=aE.z;
float row=floor((atan(aP.z/.77,aP.x)+3.1416)/.5236);float comb=(step(.85,aE.z)*step(aE.z,.95)+step(.79,aE.z)*step(aE.z,.81))*(1.-iris);float cil=pow(max(0.,sin(aP.y*5.5+t*3.4+row*1.7)),7.)*comb*(1.-Still);vW=cil*(1.+Ir);vH=fract(aP.y*.35+row*.083+t*.12);vW=mix(vW,rw*(1.+Ir)+capB*.5,bb);vH=mix(vH,fract(sr*3.+t*.05),bb);
float current=pow(max(.0,sin(t*2.2+aE.x*35.+q.y*10.+q.x*8.)),16.);vG=current+P*.9*pow(max(.0,sin(aE.x*20.+t*12.)),10.)+V*.85+cil*(.8+Ir)+Hb*1.1*iris*(1.-bb)+(1.-k)*.55+bb*(capB*.9+Hb*.8);vG*=1.-an*smoothstep(0.,.2,An)*.9;vRed=max(R*step(.964,fract(sin(aE.x*61.1+aE.y*93.7)*43758.5)),an*smoothstep(0.,.3,An));vA=(.24+aE.y*.55)*(1.-M*.18)*(1.+an*An*1.6)*(1.-an*Af)*Ga*(Mirror>.5?Mr:Mm)*1.2/(1.+blur*2.2*(1.-bb))*mix(1.,.42*(1.+2.2*capB)*mix(1.,Il,inr),bb);if(Mirror>.5)vA*=.45*Fl*exp(-max(0.,pos.y-565.)/170.)*(1.-M);gl_PointSize=clamp(aS*Pixel*(1.+vG*.18)*(1.-M*.40)*(1.+step(400.,S0)*1.4)*(1.+blur*1.6*(1.-bb))*(1.-.3*bb),1.,14.);}`;'''
NACRE = r'''vec3 nacre(float h){vec3 a=vec3(.38,.86,1.),b=vec3(.64,.56,1.),d=vec3(.46,1.,.86);return h<.333?mix(a,b,h*3.):h<.666?mix(b,d,(h-.333)*3.):mix(d,a,(h-.666)*3.);}'''
CLIP = r'''if(Hz>.5){if(vMir<.5&&vY>565.)discard;if(vMir>.5&&vY<565.)discard;}if(vMir<.5&&vY<Cl)discard;float ed=vMir<.5?exp(-max(0.,vY-Cl)/9.):0.;'''
DOTS = (r'''const dots=`precision mediump float;uniform float Hz,Cl;varying float vG,vRed,vA,vBand,vY,vW,vH,vMir;''' + NACRE +
        r'''void main(){''' + CLIP + r'''vec2 p=gl_PointCoord*2.-1.;float r=dot(p,p);if(r>1.)discard;float luminous=exp(-r*5.)*.73+exp(-r*22.)*.67;vec3 steel=vec3(.612,.765,.894),white=vec3(.956,.965,.973),red=vec3(.878,.478,.424);vec3 c=mix(steel,white,clamp(vG*.4+vBand*.32,0.,1.));c=mix(c,nacre(vH),clamp(vW,0.,1.)*.95);c=mix(c,red,vRed);c=mix(c,white,ed);gl_FragColor=vec4(c,luminous*vA*(1.+vG*.4+vW*1.3+ed*4.)*(1.-r));}`;''')
LINES = (r'''const lines=`precision mediump float;uniform float Hz,Cl;varying float vG,vRed,vA,vBand,vY,vW,vH,vMir;''' + NACRE +
         r'''void main(){''' + CLIP + r'''vec3 c=mix(vec3(.25,.43,.59),vec3(.8,.9,.99),clamp(vG*.46+vBand*.3,0.,1.));c=mix(c,nacre(vH),clamp(vW,0.,1.)*.9);c=mix(c,vec3(.878,.478,.424),vRed);c=mix(c,vec3(.956,.965,.973),ed);gl_FragColor=vec4(c,vA*(.26+.22*vG+.9*vW+ed*2.));}`;''')
FINAL = r'''const finalF=`precision highp float;varying vec2 uv;uniform sampler2D scene,bloom;uniform float T,M,Ry,Fl,Hb,Cr,Co,Sw,Sa,Sq;uniform vec2 Lc;float hash(vec2 p){return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5453);}void main(){vec2 at=vec2(uv.x*1440.,(1.-uv.y)*900.);vec3 bg=vec3(.023529,.031373,.047059);float glow=exp(-pow((at.x-Lc.x)/430.,2.)-pow((at.y-Lc.y-10.)/390.,2.));bg+=vec3(.009,.016,.022)*glow*(1.-M*.5);vec2 aw=(at-vec2(720.,430.))/vec2(980.,560.);float seam=exp(-pow((at.y-565.)/1.9,2.))*.055*(1.-M)*Fl;bg+=vec3(.4,.6,.8)*seam;vec3 a=texture2D(scene,uv).rgb,b=texture2D(bloom,uv).rgb;vec2 L=vec2(Lc.x/1440.,1.-Lc.y/900.);vec2 dd=(uv-L)/24.;vec2 pp=uv;float ww=1.;vec3 rays=vec3(0.);for(int i=0;i<24;i++){pp-=dd;rays+=texture2D(bloom,pp).rgb*ww;ww*=.92;}rays*=Ry*.065;float dc=length(at-Lc);vec3 core=vec3(.70,.86,1.)*(exp(-dc*dc/(Cr*Cr))*(.22+.75*Hb)+exp(-dc*dc/(Cr*Cr*9.))*(.05+.18*Hb))*Co;float dr=length((at-Lc)*vec2(1.,Sq));float ring=exp(-pow((dr-Sw)/(4.+Sw*.03),2.))*Sa;vec3 col=1.-exp(-(bg+a*1.28+b*1.62+rays+core+vec3(.62,.82,1.)*ring*.9)*1.12);col*=1.-.22*pow(length((uv-.5)*vec2(1.1,.85)),1.6);col+=(hash(gl_FragCoord.xy+T*13.)-.5)*.003;gl_FragColor=vec4(col,1.);}`;'''
line('const vertex=', VERTEX)
line('const dots=', DOTS)
line('const lines=', LINES)
line('const finalF=', FINAL)

# ---------- uniforms ----------
line('function paint(', r'''function paint(p,b,mode,n,mirror){gl.useProgram(p);bind(p,b);gl.uniform1f(u(p,'T'),elapsed);gl.uniform1f(u(p,'M'),morph);gl.uniform1f(u(p,'P'),pulse);gl.uniform1f(u(p,'V'),voice);gl.uniform1f(u(p,'R'),redPulse);gl.uniform1f(u(p,'Mirror'),mirror);gl.uniform1f(u(p,'Pixel'),sw/W);gl.uniform1f(u(p,'Still'),reduced?1:0);gl.uniform2f(u(p,'Eye'),look[0],look[1]);
gl.uniform1f(u(p,'As'),cine.As);gl.uniform1f(u(p,'Sp'),cine.Sp);gl.uniform1f(u(p,'Ap'),cine.Ap);gl.uniform1f(u(p,'Hb'),cine.Hb);gl.uniform1f(u(p,'Ir'),cine.Ir);gl.uniform1f(u(p,'S0'),cine.S0);gl.uniform1f(u(p,'Ga'),cine.Ga);gl.uniform1f(u(p,'Fl'),cine.fl);gl.uniform2f(u(p,'C0'),cine.cx,cine.cy);gl.uniform1f(u(p,'Hz'),cine.Hz);gl.uniform1f(u(p,'Cl'),cine.Cl);gl.uniform1f(u(p,'Mm'),cine.Mm);gl.uniform1f(u(p,'Mr'),cine.Mr);gl.uniform1f(u(p,'Df'),cine.Df);gl.uniform1f(u(p,'Bm'),cine.Bm);gl.uniform1f(u(p,'Bk'),cine.Bk);gl.uniform1f(u(p,'Av'),cine.Av);gl.uniform1f(u(p,'Ln'),p===PL?1:0);gl.uniform1f(u(p,'Cw'),cine.Cw);gl.uniform1f(u(p,'An'),cine.An);gl.uniform1f(u(p,'Af'),cine.Af);gl.uniform1f(u(p,'Il'),cine.Il);gl.uniform4f(u(p,'Rb'),720,450,413,38);gl.drawArrays(mode,0,n)}''')
line('gl.bindFramebuffer(gl.FRAMEBUFFER,null);gl.viewport(0,0,sw,sh);gl.disable(gl.BLEND);gl.useProgram(PF)',
     r'''gl.bindFramebuffer(gl.FRAMEBUFFER,null);gl.viewport(0,0,sw,sh);gl.disable(gl.BLEND);gl.useProgram(PF);bindQ(PF);tex(scene,0);tex(ta,1);gl.uniform1i(u(PF,'scene'),0);gl.uniform1i(u(PF,'bloom'),1);gl.uniform1f(u(PF,'T'),elapsed);gl.uniform1f(u(PF,'M'),morph);gl.uniform1f(u(PF,'Ry'),cine.Ry);gl.uniform1f(u(PF,'Hb'),cine.Hb);gl.uniform1f(u(PF,'Sw'),cine.Sw);gl.uniform1f(u(PF,'Sa'),cine.Sa);gl.uniform1f(u(PF,'Sq'),cine.Sq);gl.uniform1f(u(PF,'Co'),cine.Mm*(1-cine.Bm*(1-cine.Bk)));gl.uniform1f(u(PF,'Cr'),Math.max(.5,cine.Ga*cine.As*cine.Ap)*(cine.S0+(53-cine.S0)*morph)*.17);gl.uniform1f(u(PF,'Fl'),cine.fl);gl.uniform2f(u(PF,'Lc'),cine.cx+(1290-cine.cx)*morph,cine.cy+(185-cine.cy)*morph);gl.drawArrays(gl.TRIANGLE_STRIP,0,4)}''')

# uniform/attribute cache was keyed by "[object WebGLProgram]" for every program: key it per program
rep("const caches=new Map();function u(p,key){const id=p+':'+key;if(!caches.has(id))caches.set(id,key==='Q'||key[0]==='a'?gl.getAttribLocation(p,key):gl.getUniformLocation(p,key));return caches.get(id)}",
    "const caches=new WeakMap();function u(p,key){let m=caches.get(p);if(!m){m=new Map();caches.set(p,m)}if(!m.has(key))m.set(key,key==='Q'||key[0]==='a'?gl.getAttribLocation(p,key):gl.getUniformLocation(p,key));return m.get(key)}")

# no mirror at all: no reflected pass, no reflected bar, no floor
rep("if(morph<.95){paint(PL,bl,gl.LINES,Math.floor(segs.length/7*.45),1);paint(PD,bp,gl.POINTS,Math.floor(pointDraw*.55),1)}","")
rep(".caption{display:none}#stage.open .mast{opacity:0}", ".caption{display:none}#stage.open .mast{opacity:0}.lens-reflect,.floor{display:none!important}")
rep("float seam=exp(-pow((at.y-565.)/1.9,2.))*.055*(1.-M)*Fl;","float seam=0.;")

# ---------- the director (timeline) ----------
DIRECTOR = open(__file__.replace('patch.py', 'director.js'), encoding='utf-8').read()
rep("// NÁCAR: Pure WebGL 1", DIRECTOR + "\n// NÁCAR: Pure WebGL 1")
# advance the entrance every frame (before anything else draws)
rep("elapsed+=dt;const q=1-Math.exp(-dt*4.2);", "elapsed+=dt;cineFrame(ms);const q=1-Math.exp(-dt*4.2);")

exec(open(__file__.replace('patch.py','planeta.py'),encoding='utf-8').read())
exec(open(__file__.replace('patch.py','selector.py'),encoding='utf-8').read())
exec(open(__file__.replace('patch.py','ultra.py'),encoding='utf-8').read())
exec(open(__file__.replace('patch.py','producto.py'),encoding='utf-8').read())
open(dst, 'w', encoding='utf-8').write(s)
print('ok', len(s))
