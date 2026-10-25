// (SP2) Cuts Elden Ring's own title logo (MENU_Title_EldenRing_01 of atlas SB_Title_01, /menu/hi/01_common.tpf of the
// user's install, already extracted read-only by tools/er_icons.js into build/sora_tmp/er/er43/game) and lays it on a
// 512x400 transparent canvas like KH3's MenuWorldTitle* pictures. Args: OUTDIR
const fs=require('fs'),path=require('path');
const ROOT=process.env.KH_PACKGEN_ROOT.slice(0, -1);
const bc=require(ROOT+'/tools/lib/bcdec');
const out=process.argv[2];
const b=fs.readFileSync(ROOT+'/build/sora_tmp/er/er43/game/menu/hi/01_common.tpf');
const lyt=fs.readFileSync(ROOT+'/build/sora_tmp/er/er43/game/menu/hi/01_common.sblytbnd').toString('latin1');
const m=/<SubTexture name="MENU_Title_EldenRing_01\.png" x="(\d+)" y="(\d+)" width="(\d+)" height="(\d+)"/.exec(lyt);
const [sx,sy,sw,sh]=m.slice(1).map(Number);
let dds=null;const n=b.readInt32LE(8);
for(let k=0;k<n;k++){const o=0x10+k*0x14;const off=b.readUInt32LE(o),size=b.readInt32LE(o+4),no=b.readUInt32LE(o+12);let e=no;while(b.readUInt16LE(e)!==0)e+=2;if(b.slice(no,e).toString('utf16le')==='SB_Title_01')dds=b.slice(off,off+size);}
const t=bc.decodeDds(dds,0);
console.log('atlas',t.width,t.height,t.format,'cut',sx,sy,sw,sh);
// the cut, full size
const cut=Buffer.alloc(sw*sh*4);
for(let y=0;y<sh;y++)t.rgba.copy(cut,y*sw*4,((sy+y)*t.width+sx)*4,((sy+y)*t.width+sx+sw)*4);
fs.mkdirSync(out,{recursive:true});
fs.writeFileSync(path.join(out,'MENU_Title_EldenRing_01_full.png'),bc.encodePng(sw,sh,cut,{alpha:true}));
// THE GAME DRAWS IT ADDITIVELY (an opaque picture on black): the same light as straight alpha = alpha max(r,g,b), colour / alpha
let low=0;for(let i=0;i<sw*sh;i++)if(cut[i*4+3]<224)low++;const opaque=low===0;console.log('opaque source (additive):',opaque);
if(opaque)for(let i=0;i<sw*sh;i++){const p=i*4,a=Math.max(cut[p],cut[p+1],cut[p+2]);cut[p+3]=a;if(a>0){cut[p]=Math.round(cut[p]*255/a);cut[p+1]=Math.round(cut[p+1]*255/a);cut[p+2]=Math.round(cut[p+2]*255/a);}}
// tight bounds of what is drawn (alpha > 8)
let x0=sw,y0=sh,x1=0,y1=0;
for(let y=0;y<sh;y++)for(let x=0;x<sw;x++)if(cut[(y*sw+x)*4+3]>24){if(x<x0)x0=x;if(x>x1)x1=x;if(y<y0)y0=y;if(y>y1)y1=y;}
console.log('drawn bounds',x0,y0,x1,y1);
// fit into KH3's world-title box: canvas 512x400, the logo within 470x330 centred (KH3's Olympus: ~460 wide, ~300 tall)
const W=512,H=400,BW=+process.argv[3]||470,BH=+process.argv[4]||330;
const cw=x1-x0+1,ch=y1-y0+1,s=Math.min(BW/cw,BH/ch);
const dw=Math.round(cw*s),dh=Math.round(ch*s),ox=Math.round((W-dw)/2),oy=Math.round((H-dh)/2);
const pic=Buffer.alloc(W*H*4);
for(let y=0;y<dh;y++)for(let x=0;x<dw;x++){
  // box filter over the source footprint, premultiplied
  const fx0=x0+x/s,fx1=x0+(x+1)/s,fy0=y0+y/s,fy1=y0+(y+1)/s;let r=0,g=0,bl=0,a=0,c=0;
  for(let yy=Math.floor(fy0);yy<Math.ceil(fy1);yy++)for(let xx=Math.floor(fx0);xx<Math.ceil(fx1);xx++){const p=(yy*sw+xx)*4,al=cut[p+3]/255;r+=cut[p]*al;g+=cut[p+1]*al;bl+=cut[p+2]*al;a+=al;c++;}
  const q=((oy+y)*W+ox+x)*4;if(a>0){pic[q]=Math.round(r/a);pic[q+1]=Math.round(g/a);pic[q+2]=Math.round(bl/a);}pic[q+3]=Math.round(255*a/c);
}
fs.writeFileSync(path.join(out,'MenuWorldTitleER.png'),bc.encodePng(W,H,pic,{alpha:true}));
console.log('scale',s.toFixed(4),'drawn',dw,dh,'at',ox,oy);
