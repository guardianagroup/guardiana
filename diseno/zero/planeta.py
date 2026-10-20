# The PLANET opening: the real Earth (Órbita's day texture), alive, then cosmic dust or falling sand.
EARTHF = r'''const earthF=`precision highp float;varying vec2 uv;uniform sampler2D tex;uniform vec2 Ce;uniform float Re,Er,Eo,Ds;
float h(vec2 p){return fract(sin(dot(p,vec2(12.9898,78.233)))*43758.5453);}
float vn(vec2 p){vec2 i=floor(p),f=fract(p);f=f*f*(3.-2.*f);return mix(mix(h(i),h(i+vec2(1.,0.)),f.x),mix(h(i+vec2(0.,1.)),h(i+1.),f.x),f.y);}
void main(){vec2 at=vec2(uv.x*1440.,(1.-uv.y)*900.);vec2 d=(at-Ce)/Re;float r=length(d);
float nz=vn(at*.32)*.35+vn(at*.09)*.4+vn(at*.025)*.25;float nz2=nz*.35+.65*(1.-(clamp(d.y,-1.,1.)+1.)*.5);nz=nz2;float keep=Ds<.001?1.:smoothstep(Ds-.03,Ds+.03,nz);
float halo=exp(-pow((r-1.)/.04,2.))*.5+exp(-max(r-1.,0.)*7.)*.16*step(1.,r);
if(r>1.){gl_FragColor=vec4(vec3(.62,.72,.82)*halo*.55*Eo*keep,0.);return;}
vec3 n=vec3(d.x,-d.y,sqrt(1.-r*r));float tl=-.2;vec3 m=vec3(n.x,n.y*cos(tl)-n.z*sin(tl),n.y*sin(tl)+n.z*cos(tl));
float lo=atan(m.x,m.z)+Er;float la=asin(clamp(m.y,-1.,1.));vec2 tc=vec2(fract(lo/6.28318+.5),clamp(.5-la/3.14159,0.,1.));
vec3 col=texture2D(tex,tc).rgb;vec3 sun=normalize(vec3(-.55,.35,.76));float dif=clamp(dot(n,sun),0.,1.);
col=col*(.04+1.25*pow(dif,.8))+vec3(.55,.66,.78)*pow(1.-n.z,4.)*.12;
float edge=(1.-smoothstep(Ds,Ds+.04,nz))*step(.001,Ds)*keep;col+=vec3(.7,.88,1.)*edge*1.5;
gl_FragColor=vec4(col*.9*Eo*keep,Eo*keep);}`;'''

# vertex: earth attributes and uniforms
rep("attribute float aS;uniform float T,M,P,V,R,", "attribute float aS;attribute vec2 aL;attribute vec3 aK;uniform vec2 Ce;uniform float Re,Er,Du,Pv,Fa;uniform float T,M,P,V,R,")
rep("varying float vG,vRed,vA,vBand,vY,vW,vH,vMir;vec4 rrect", "varying float vG,vRed,vA,vBand,vY,vW,vH,vMir;varying vec3 vEc;varying float vEw;vec4 rrect")
s_count = s.count("uniform float Hz,Cl;varying float vG,vRed,vA,vBand,vY,vW,vH,vMir;")
assert s_count == 2, s_count
s = s.replace("uniform float Hz,Cl;varying float vG,vRed,vA,vBand,vY,vW,vH,vMir;", "uniform float Hz,Cl;varying float vG,vRed,vA,vBand,vY,vW,vH,vMir;varying vec3 vEc;varying float vEw;")
rep("q=mix(qsp,qsc,Av);", "q=mix(qsp,qsc,clamp(Av,0.,1.));if(Av>1.5)q=aP;")
# earth / dust / falling sand, in screen space
rep("pos.x+=sign(q.x)*Sp;float grp=",
    "pos.x+=sign(q.x)*Sp;"
    "float tl=-.2;float lo=aL.x-Er,la=aL.y;vec3 m3=vec3(cos(la)*sin(lo),sin(la),cos(la)*cos(lo));vec3 n3=vec3(m3.x,m3.y*cos(tl)+m3.z*sin(tl),-m3.y*sin(tl)+m3.z*cos(tl));"
    "float rq=fract(aE.y*13.7);float front=step(0.,n3.z);"
    "vec3 nd=n3*(1.+Du*(.35+1.9*rq*rq));float sa=Du*(1.2+rq)*.9;nd=vec3(nd.x*cos(sa)+nd.z*sin(sa),nd.y+Du*sin(T*.5+rq*20.)*.12,-nd.x*sin(sa)+nd.z*cos(sa));"
    "float ppe=3./(3.-nd.z*.9);vec2 epos=Ce+vec2(nd.x,-nd.y)*Re*ppe;float st=0.;"
    "float dl=(n3.y+1.)*.5*2.5+rq*.12;float rel=1.;if(Av>2.5){float tf=max(0.,Fa-dl);rel=smoothstep(dl-.06,dl+.1,Fa);vec2 e0=Ce+vec2(n3.x,-n3.y)*Re;epos=vec2(e0.x+sin(rq*40.)*tf*9.,e0.y+.5*560.*tf*tf);}"
    "float isE=step(1.5,Av);float vis=mix(front,1.,clamp(Du*3.+step(.001,Fa),0.,1.));"
    "if(isE>.5&&Av<2.5)pos=mix(epos,pos,k);"
    "float grp=")
rep("pos=mix(pos,rim,bb);",
    "if(Av>2.5){float y0=Ce.y-Re;float fall=step(.001,Fa);float land=fall*smoothstep(rim.y-55.,rim.y+4.,epos.y);epos.x=mix(epos.x,rim.x,fall*smoothstep(0.,1.,(epos.y-y0)/(rim.y-y0+1.)));pos=epos;bb=land*(1.-M);st=land;}"
    "pos=mix(pos,rim,bb);")
rep("*mix(1.,.42*(1.+2.2*capB)*mix(1.,Il,inr),bb);",
    "*mix(1.,.42*(1.+2.2*capB)*mix(1.,Il,inr),bb);vA*=mix(1.,Pv*vis*rel,isE*(1.-max(k,st)))*mix(1.,k,isE*Ln*step(Av,2.5))*mix(1.,0.,step(2.5,Av)*Ln*(1.-st));vEc=aK;vEw=isE*(1.-max(k,st));vA*=1.+isE*(1.-k)*Du*2.2;")
# dots take the Earth's colour while they are dust or sand
rep("c=mix(c,nacre(vH),clamp(vW,0.,1.)*.95);", "c=mix(c,nacre(vH),clamp(vW,0.,1.)*.95);c=mix(c,pow(vEc,vec3(.8))*1.35+.04,vEw);")

# earth program, texture, per-particle Earth coordinates and colours
rep("const PD=make(vertex,dots),", EARTHF + "\nconst PE=make(quadV,earthF);let earthTex=null;\nconst PD=make(vertex,dots),")
rep("const segs=new Float32Array(arr);",
    "const segs=new Float32Array(arr);\n"
    "const earthD=new Float32Array(maxPts*5);for(let i=0;i<maxPts;i++){earthD[i*5]=(Math.random()-.5)*Math.PI*2;earthD[i*5+1]=Math.asin(2*Math.random()-1);earthD[i*5+2]=.61;earthD[i*5+3]=.76;earthD[i*5+4]=.89}const eb=buf(earthD);\n"
    "function sampleEarth(im){try{const c=document.createElement('canvas');c.width=1024;c.height=512;const x=c.getContext('2d');x.drawImage(im,0,0,1024,512);const d=x.getImageData(0,0,1024,512).data;for(let i=0;i<maxPts;i++){const uu=earthD[i*5]/(Math.PI*2)+.5,vv=.5-earthD[i*5+1]/Math.PI;const px=Math.min(1023,Math.max(0,Math.floor(uu*1024))),py=Math.min(511,Math.max(0,Math.floor(vv*512))),o=(py*1024+px)*4;earthD[i*5+2]=d[o]/255;earthD[i*5+3]=d[o+1]/255;earthD[i*5+4]=d[o+2]/255}gl.bindBuffer(gl.ARRAY_BUFFER,eb);gl.bufferData(gl.ARRAY_BUFFER,earthD,gl.STATIC_DRAW)}catch(e){}}\n"
    "{const im=new Image();im.onload=()=>{try{earthTex=gl.createTexture();gl.bindTexture(gl.TEXTURE_2D,earthTex);gl.texImage2D(gl.TEXTURE_2D,0,gl.RGB,gl.RGB,gl.UNSIGNED_BYTE,im);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MIN_FILTER,gl.LINEAR);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MAG_FILTER,gl.LINEAR);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_S,gl.CLAMP_TO_EDGE);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_T,gl.CLAMP_TO_EDGE)}catch(e){earthTex=null}sampleEarth(im)};/* planet texture not used */0&&(im.src='')}\n"
    "function drawEarth(){if(!earthTex||cine.Eo<.002)return;gl.useProgram(PE);bindQ(PE);gl.activeTexture(gl.TEXTURE0);gl.bindTexture(gl.TEXTURE_2D,earthTex);gl.uniform1i(u(PE,'tex'),0);gl.uniform2f(u(PE,'Ce'),cine.ex,cine.ey);gl.uniform1f(u(PE,'Re'),cine.Re);gl.uniform1f(u(PE,'Er'),cine.Er);gl.uniform1f(u(PE,'Eo'),cine.Eo);gl.uniform1f(u(PE,'Ds'),cine.Ds);gl.blendFunc(gl.ONE,gl.ONE_MINUS_SRC_ALPHA);gl.drawArrays(gl.TRIANGLE_STRIP,0,4);gl.blendFunc(gl.SRC_ALPHA,gl.ONE)}")
rep("function bind(p,b){",
    "function bind(p,b){bind0(p,b);const aL=u(p,'aL'),aK=u(p,'aK');if(aL<0||aK<0)return;if(b===bp){gl.bindBuffer(gl.ARRAY_BUFFER,eb);gl.enableVertexAttribArray(aL);gl.vertexAttribPointer(aL,2,gl.FLOAT,false,20,0);gl.enableVertexAttribArray(aK);gl.vertexAttribPointer(aK,3,gl.FLOAT,false,20,8)}else{gl.disableVertexAttribArray(aL);gl.disableVertexAttribArray(aK)}}\nfunction bind0(p,b){")
rep("paint(PL,bl,gl.LINES,segs.length/7,0);paint(PD", "drawEarth();paint(PL,bl,gl.LINES,segs.length/7,0);paint(PD")
rep("gl.uniform4f(u(p,'Rb'),720,450,413,38);", "gl.uniform4f(u(p,'Rb'),720,450,413,38);gl.uniform2f(u(p,'Ce'),cine.ex,cine.ey);gl.uniform1f(u(p,'Re'),cine.Re);gl.uniform1f(u(p,'Er'),cine.Er);gl.uniform1f(u(p,'Du'),cine.Du);gl.uniform1f(u(p,'Pv'),cine.Pv);gl.uniform1f(u(p,'Fa'),cine.Fa);")
rep("*(1.-.3*bb),1.,14.);","*(1.-.3*bb)*(1.+isE*(1.-k)*Du*.9),1.,14.);")
rep("float isE=step(1.5,Av);float vis=", "if(Av<2.5&&Fa>0.){float tf2=max(0.,Fa-rq*.6);epos.y+=min(.5*520.*tf2*tf2,330.);epos.x+=sin(rq*31.)*tf2*10.;}float isE=step(1.5,Av);float vis=")
