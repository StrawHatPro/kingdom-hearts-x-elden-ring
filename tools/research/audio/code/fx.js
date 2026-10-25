// Function index for the audio/code strand (cache in fx_cache.json, built on first run from sheets, read-only).
// usage: node fx.js info ADDR...        name size ncallers verdict
//        node fx.js callers ADDR...     callers (through jump thunks too) with names
//        node fx.js callees ADDR
//        node fx.js range LO HI         functions in [LO,HI)
//        node fx.js name REGEX          functions by name
const fs=require('fs'),path=require('path');const R=path.resolve(__dirname,'../../../..');
const CF=path.join(__dirname,'fx_cache.json');
function rows(f,cb){for(const l of fs.readFileSync(path.join(R,f),'utf8').split('\n')){if(l[0]!=='[')continue;let r;try{r=JSON.parse(l.replace(/,\s*$/,'').replace(/\]\}\s*$/,''));}catch(e){continue;}cb(r);}}
function build(){const F={};rows('sheets/native/code.json',r=>{F[r[0].toLowerCase()]={n:r[1],s:r[2],t:r[5]==='yes'?1:0};});
 rows('sheets/impl/progress.json',r=>{const f=F[r[0].toLowerCase()];if(f){f.v=r[6];if(r[2]&&/^(FUN_|thunk_FUN_)/.test(f.n))f.n=r[2];}});
 rows('sheets/exe/names.json',r=>{const f=F[r[1].toLowerCase()];if(f&&/^(FUN_|thunk_FUN_)/.test(f.n))f.n=r[2];});
 rows('sheets/exe/vfuncs.json',r=>{const f=F[String(r[1]).toLowerCase()];if(f&&r[2]&&r[2]!=='-'){f.vf=(f.vf?f.vf+'|':'')+r[2]+'#'+r[4];}});
 const C={};rows('sheets/exe/calls.json',r=>{C[r[0].toLowerCase()]=r[1].map(x=>x.toLowerCase());});
 fs.writeFileSync(CF,JSON.stringify({F,C}));return {F,C};}
let D;function load(){if(D)return D;D=fs.existsSync(CF)?JSON.parse(fs.readFileSync(CF,'utf8')):build();return D;}
function norm(a){a=String(a).toLowerCase();return a.startsWith('0x')?a:'0x'+a;}
function label(a){const f=load().F[a];return a+' '+(f?`${f.n} [${f.s}b v=${f.v||'?'}${f.vf?' vf='+f.vf.slice(0,80):''}${f.t?' thunk':''}]`:'?');}
let REV;function rev(){if(REV)return REV;REV={};const {C}=load();for(const k in C)for(const c of C[k])(REV[c]=REV[c]||[]).push(k);return REV;}
function callers(a,depth=0){a=norm(a);const {F}=load();const out=[];for(const c of (rev()[a]||[])){const f=F[c];if(f&&f.t&&depth<3)out.push(...callers(c,depth+1));else out.push(c);}return [...new Set(out)];}
module.exports={load,norm,label,callers,rev};
if(require.main===module){const [cmd,...a]=process.argv.slice(2);const {F,C}=load();
 if(cmd==='info')for(const x of a)console.log(label(norm(x)));
 else if(cmd==='callers')for(const x of a){const cs=callers(x);console.log('## callers of '+label(norm(x))+' : '+cs.length);for(const c of cs.sort())console.log('  '+label(c));}
 else if(cmd==='callees')for(const x of a){console.log('## callees of '+label(norm(x)));for(const c of (C[norm(x)]||[]))console.log('  '+label(c));}
 else if(cmd==='range'){const lo=parseInt(a[0],16),hi=parseInt(a[1],16);for(const k of Object.keys(F)){const v=parseInt(k,16);if(v>=lo&&v<hi)console.log(label(k)+' callers='+((rev()[k]||[]).length));}}
 else if(cmd==='name'){const re=new RegExp(a[0]);for(const k of Object.keys(F))if(re.test(F[k].n))console.log(label(k));}
 else if(cmd==='build')build();}
